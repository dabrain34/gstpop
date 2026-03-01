/*
 * GStreamer Prince of Parser
 *
 * Copyright (C) 2020-2024 Stéphane Cerveau <scerveau@igalia.com>
 *
 * SPDX-License-Identifier: LGPL-2.1-or-later
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation
 * version 2.1 of the License.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library; if not, write to the Free Software
 * Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301 USA
 *
 */

#include "gstpop-client.h"
#include <string.h>

struct _GSTPOPClient
{
  GObject parent;

  SoupSession *session;
  SoupWebsocketConnection *ws;
  gchar *url;
  gboolean connected;

  GSTPOPClientResponseCallback response_callback;
  gpointer response_user_data;

  GSTPOPClientErrorCallback error_callback;
  gpointer error_user_data;

  GSTPOPClientEventCallback event_callback;
  gpointer event_user_data;

  GSTPOPClientConnectedCallback connected_callback;
  gpointer connected_user_data;

  GSTPOPClientClosedCallback closed_callback;
  gpointer closed_user_data;
};

G_DEFINE_TYPE (GSTPOPClient, gstpop_client, G_TYPE_OBJECT)

static gchar *
generate_uuid (void)
{
  return g_uuid_string_random ();
}

gchar *
gstpop_json_to_pretty_string (JsonNode *node)
{
  g_return_val_if_fail (node != NULL, g_strdup ("null"));

  JsonGenerator *gen = json_generator_new ();
  json_generator_set_pretty (gen, TRUE);
  json_generator_set_indent (gen, 2);
  json_generator_set_root (gen, node);
  gchar *str = json_generator_to_data (gen, NULL);
  g_object_unref (gen);
  return str;
}

static void
handle_event (GSTPOPClient *client, JsonObject *root)
{
  if (!client->event_callback)
    return;

  const gchar *event_type = json_object_get_string_member (root, "event");
  if (!event_type) {
    g_warning ("Event message missing 'event' string field");
    return;
  }
  JsonNode *data_node = json_object_get_member (root, "data");

  client->event_callback (client, event_type, data_node, client->event_user_data);
}

static void
handle_response (GSTPOPClient *client, JsonObject *root)
{
  const gchar *id = json_object_get_string_member (root, "id");
  if (!id) {
    g_warning ("Response message missing string 'id' field");
    return;
  }

  if (json_object_has_member (root, "error")) {
    if (client->error_callback) {
      JsonObject *error = json_object_get_object_member (root, "error");
      gint64 code = json_object_get_int_member (error, "code");
      const gchar *message = json_object_get_string_member (error, "message");
      client->error_callback (client, id, (gint) code, message, client->error_user_data);
    }
  } else if (json_object_has_member (root, "result")) {
    if (client->response_callback) {
      JsonNode *result_node = json_object_get_member (root, "result");
      client->response_callback (client, id, result_node, client->response_user_data);
    }
  }
}

static void
process_message (GSTPOPClient *client, const gchar *text)
{
  JsonParser *parser = json_parser_new ();
  GError *error = NULL;

  if (!json_parser_load_from_data (parser, text, -1, &error)) {
    g_warning ("Failed to parse JSON message: %s", error->message);
    g_error_free (error);
    g_object_unref (parser);
    return;
  }

  JsonNode *root_node = json_parser_get_root (parser);
  if (root_node == NULL) {
    g_warning ("JSON parser returned NULL root node");
    g_object_unref (parser);
    return;
  }

  JsonObject *root = json_node_get_object (root_node);
  if (root == NULL) {
    g_warning ("JSON root is not an object");
    g_object_unref (parser);
    return;
  }

  if (json_object_has_member (root, "event")) {
    handle_event (client, root);
  } else if (json_object_has_member (root, "id")) {
    handle_response (client, root);
  }

  g_object_unref (parser);
}

static void
on_websocket_message (SoupWebsocketConnection *ws,
                      SoupWebsocketDataType type,
                      GBytes *message,
                      gpointer user_data)
{
  GSTPOPClient *client = GSTPOP_CLIENT (user_data);
  (void) ws;

  if (type == SOUP_WEBSOCKET_DATA_TEXT) {
    gsize len;
    const gchar *data = g_bytes_get_data (message, &len);
    gchar *text = g_strndup (data, len);
    process_message (client, text);
    g_free (text);
  }
}

static void
on_websocket_closed (SoupWebsocketConnection *ws, gpointer user_data)
{
  GSTPOPClient *client = GSTPOP_CLIENT (user_data);
  (void) ws;

  client->connected = FALSE;

  if (client->closed_callback) {
    client->closed_callback (client, client->closed_user_data);
  }
}

static void
on_websocket_error (SoupWebsocketConnection *ws,
                    GError *error,
                    gpointer user_data)
{
  GSTPOPClient *client = GSTPOP_CLIENT (user_data);
  (void) ws;

  g_warning ("WebSocket error: %s", error->message);
  if (client->error_callback) {
    client->error_callback (client, NULL, -1, error->message, client->error_user_data);
  }
}

static void
on_websocket_connected (GObject *source,
                        GAsyncResult *result,
                        gpointer user_data)
{
  GSTPOPClient *client = GSTPOP_CLIENT (user_data);
  GError *error = NULL;

  client->ws = soup_session_websocket_connect_finish (SOUP_SESSION (source),
                                                       result,
                                                       &error);

  if (error) {
    client->connected = FALSE;
    if (client->connected_callback) {
      client->connected_callback (client, FALSE, error->message, client->connected_user_data);
    }
    g_error_free (error);
    return;
  }

  client->connected = TRUE;

  g_signal_connect (client->ws, "message",
                    G_CALLBACK (on_websocket_message), client);
  g_signal_connect (client->ws, "closed",
                    G_CALLBACK (on_websocket_closed), client);
  g_signal_connect (client->ws, "error",
                    G_CALLBACK (on_websocket_error), client);

  if (client->connected_callback) {
    client->connected_callback (client, TRUE, NULL, client->connected_user_data);
  }
}

static void
gstpop_client_dispose (GObject *object)
{
  GSTPOPClient *client = GSTPOP_CLIENT (object);

  if (client->ws) {
    if (soup_websocket_connection_get_state (client->ws) == SOUP_WEBSOCKET_STATE_OPEN) {
      soup_websocket_connection_close (client->ws, SOUP_WEBSOCKET_CLOSE_NORMAL, NULL);
    }
    g_clear_object (&client->ws);
  }

  g_clear_object (&client->session);
  g_clear_pointer (&client->url, g_free);

  G_OBJECT_CLASS (gstpop_client_parent_class)->dispose (object);
}

static void
gstpop_client_class_init (GSTPOPClientClass *klass)
{
  GObjectClass *object_class = G_OBJECT_CLASS (klass);
  object_class->dispose = gstpop_client_dispose;
}

static void
gstpop_client_init (GSTPOPClient *client)
{
  client->session = soup_session_new ();
  client->connected = FALSE;
}

/* Public API */

GSTPOPClient *
gstpop_client_new (const gchar *url)
{
  GSTPOPClient *client = g_object_new (GSTPOP_TYPE_CLIENT, NULL);
  client->url = g_strdup (url ? url : GSTPOP_CLIENT_DEFAULT_URL);
  return client;
}

void
gstpop_client_free (GSTPOPClient *client)
{
  g_clear_object (&client);
}

void
gstpop_client_set_response_callback (GSTPOPClient *client,
                                   GSTPOPClientResponseCallback callback,
                                   gpointer user_data)
{
  g_return_if_fail (GSTPOP_IS_CLIENT (client));
  client->response_callback = callback;
  client->response_user_data = user_data;
}

void
gstpop_client_set_error_callback (GSTPOPClient *client,
                                GSTPOPClientErrorCallback callback,
                                gpointer user_data)
{
  g_return_if_fail (GSTPOP_IS_CLIENT (client));
  client->error_callback = callback;
  client->error_user_data = user_data;
}

void
gstpop_client_set_event_callback (GSTPOPClient *client,
                                GSTPOPClientEventCallback callback,
                                gpointer user_data)
{
  g_return_if_fail (GSTPOP_IS_CLIENT (client));
  client->event_callback = callback;
  client->event_user_data = user_data;
}

void
gstpop_client_set_connected_callback (GSTPOPClient *client,
                                    GSTPOPClientConnectedCallback callback,
                                    gpointer user_data)
{
  g_return_if_fail (GSTPOP_IS_CLIENT (client));
  client->connected_callback = callback;
  client->connected_user_data = user_data;
}

void
gstpop_client_set_closed_callback (GSTPOPClient *client,
                                 GSTPOPClientClosedCallback callback,
                                 gpointer user_data)
{
  g_return_if_fail (GSTPOP_IS_CLIENT (client));
  client->closed_callback = callback;
  client->closed_user_data = user_data;
}

void
gstpop_client_connect (GSTPOPClient *client)
{
  g_return_if_fail (GSTPOP_IS_CLIENT (client));

  SoupMessage *msg = soup_message_new (SOUP_METHOD_GET, client->url);
  if (!msg) {
    if (client->connected_callback) {
      client->connected_callback (client, FALSE, "Invalid URL", client->connected_user_data);
    }
    return;
  }

  soup_session_websocket_connect_async (client->session,
                                        msg,
                                        NULL,
                                        NULL,
                                        G_PRIORITY_DEFAULT,
                                        NULL,
                                        on_websocket_connected,
                                        client);
  g_object_unref (msg);
}

void
gstpop_client_disconnect (GSTPOPClient *client)
{
  g_return_if_fail (GSTPOP_IS_CLIENT (client));

  if (client->ws && soup_websocket_connection_get_state (client->ws) == SOUP_WEBSOCKET_STATE_OPEN) {
    soup_websocket_connection_close (client->ws, SOUP_WEBSOCKET_CLOSE_NORMAL, NULL);
  }
}

gboolean
gstpop_client_is_connected (GSTPOPClient *client)
{
  g_return_val_if_fail (GSTPOP_IS_CLIENT (client), FALSE);
  return client->connected;
}

gchar *
gstpop_client_send_request (GSTPOPClient *client,
                          const gchar *method,
                          JsonObject *params)
{
  g_return_val_if_fail (GSTPOP_IS_CLIENT (client), NULL);
  g_return_val_if_fail (method != NULL, NULL);

  if (!client->connected || !client->ws) {
    return NULL;
  }

  gchar *uuid = generate_uuid ();

  JsonBuilder *builder = json_builder_new ();
  json_builder_begin_object (builder);

  json_builder_set_member_name (builder, "id");
  json_builder_add_string_value (builder, uuid);

  json_builder_set_member_name (builder, "method");
  json_builder_add_string_value (builder, method);

  json_builder_set_member_name (builder, "params");
  if (params) {
    JsonNode *params_node = json_node_new (JSON_NODE_OBJECT);
    json_node_set_object (params_node, params);
    json_builder_add_value (builder, params_node);
  } else {
    json_builder_begin_object (builder);
    json_builder_end_object (builder);
  }

  json_builder_end_object (builder);

  JsonGenerator *gen = json_generator_new ();
  JsonNode *root = json_builder_get_root (builder);
  json_generator_set_root (gen, root);
  gchar *json_str = json_generator_to_data (gen, NULL);

  soup_websocket_connection_send_text (client->ws, json_str);

  g_free (json_str);
  json_node_unref (root);
  g_object_unref (gen);
  g_object_unref (builder);

  return uuid;
}

/* Convenience methods */

gchar *
gstpop_client_list_pipelines (GSTPOPClient *client)
{
  return gstpop_client_send_request (client, "list_pipelines", NULL);
}

gchar *
gstpop_client_create_pipeline (GSTPOPClient *client, const gchar *description)
{
  JsonObject *params = json_object_new ();
  json_object_set_string_member (params, "description", description);
  gchar *id = gstpop_client_send_request (client, "create_pipeline", params);
  json_object_unref (params);
  return id;
}

gchar *
gstpop_client_update_pipeline (GSTPOPClient *client, const gchar *pipeline_id, const gchar *description)
{
  JsonObject *params = json_object_new ();
  json_object_set_string_member (params, "pipeline_id", pipeline_id);
  json_object_set_string_member (params, "description", description);
  gchar *id = gstpop_client_send_request (client, "update_pipeline", params);
  json_object_unref (params);
  return id;
}

gchar *
gstpop_client_remove_pipeline (GSTPOPClient *client, const gchar *pipeline_id)
{
  JsonObject *params = json_object_new ();
  json_object_set_string_member (params, "pipeline_id", pipeline_id);
  gchar *id = gstpop_client_send_request (client, "remove_pipeline", params);
  json_object_unref (params);
  return id;
}

gchar *
gstpop_client_get_pipeline_info (GSTPOPClient *client, const gchar *pipeline_id)
{
  JsonObject *params = json_object_new ();
  json_object_set_string_member (params, "pipeline_id", pipeline_id);
  gchar *id = gstpop_client_send_request (client, "get_pipeline_info", params);
  json_object_unref (params);
  return id;
}

gchar *
gstpop_client_play (GSTPOPClient *client, const gchar *pipeline_id)
{
  JsonObject *params = NULL;
  if (pipeline_id) {
    params = json_object_new ();
    json_object_set_string_member (params, "pipeline_id", pipeline_id);
  }
  gchar *id = gstpop_client_send_request (client, "play", params);
  if (params)
    json_object_unref (params);
  return id;
}

gchar *
gstpop_client_pause (GSTPOPClient *client, const gchar *pipeline_id)
{
  JsonObject *params = NULL;
  if (pipeline_id) {
    params = json_object_new ();
    json_object_set_string_member (params, "pipeline_id", pipeline_id);
  }
  gchar *id = gstpop_client_send_request (client, "pause", params);
  if (params)
    json_object_unref (params);
  return id;
}

gchar *
gstpop_client_stop (GSTPOPClient *client, const gchar *pipeline_id)
{
  JsonObject *params = NULL;
  if (pipeline_id) {
    params = json_object_new ();
    json_object_set_string_member (params, "pipeline_id", pipeline_id);
  }
  gchar *id = gstpop_client_send_request (client, "stop", params);
  if (params)
    json_object_unref (params);
  return id;
}

gchar *
gstpop_client_set_state (GSTPOPClient *client, const gchar *pipeline_id, const gchar *state)
{
  JsonObject *params = json_object_new ();
  json_object_set_string_member (params, "pipeline_id", pipeline_id);
  json_object_set_string_member (params, "state", state);
  gchar *id = gstpop_client_send_request (client, "set_state", params);
  json_object_unref (params);
  return id;
}

gchar *
gstpop_client_snapshot (GSTPOPClient *client, const gchar *pipeline_id, const gchar *details)
{
  JsonObject *params = json_object_new ();
  if (pipeline_id) {
    json_object_set_string_member (params, "pipeline_id", pipeline_id);
  }
  if (details) {
    json_object_set_string_member (params, "details", details);
  }
  gchar *id = gstpop_client_send_request (client, "snapshot", params);
  json_object_unref (params);
  return id;
}

gchar *
gstpop_client_get_position (GSTPOPClient *client, const gchar *pipeline_id)
{
  JsonObject *params = NULL;
  if (pipeline_id) {
    params = json_object_new ();
    json_object_set_string_member (params, "pipeline_id", pipeline_id);
  }
  gchar *id = gstpop_client_send_request (client, "get_position", params);
  if (params)
    json_object_unref (params);
  return id;
}

gchar *
gstpop_client_get_version (GSTPOPClient *client)
{
  return gstpop_client_send_request (client, "get_version", NULL);
}

gchar *
gstpop_client_get_info (GSTPOPClient *client)
{
  return gstpop_client_send_request (client, "get_info", NULL);
}

gchar *
gstpop_client_get_pipeline_count (GSTPOPClient *client)
{
  return gstpop_client_send_request (client, "get_pipeline_count", NULL);
}

gchar *
gstpop_client_get_elements (GSTPOPClient *client, const gchar *detail)
{
  JsonObject *params = NULL;
  if (detail) {
    params = json_object_new ();
    json_object_set_string_member (params, "detail", detail);
  }
  gchar *id = gstpop_client_send_request (client, "get_elements", params);
  if (params)
    json_object_unref (params);
  return id;
}

gchar *
gstpop_client_discover_uri (GSTPOPClient *client, const gchar *uri, guint timeout)
{
  JsonObject *params = json_object_new ();
  json_object_set_string_member (params, "uri", uri);
  if (timeout > 0) {
    json_object_set_int_member (params, "timeout", (gint64) timeout);
  }
  gchar *id = gstpop_client_send_request (client, "discover_uri", params);
  json_object_unref (params);
  return id;
}
