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

#ifndef _GSTPOP_CLIENT_H_
#define _GSTPOP_CLIENT_H_

#include <glib.h>
#include <gio/gio.h>
#include <libsoup/soup.h>
#include <json-glib/json-glib.h>

G_BEGIN_DECLS

#define GSTPOP_TYPE_CLIENT (gstpop_client_get_type())
G_DECLARE_FINAL_TYPE (GSTPOPClient, gstpop_client, GSTPOP, CLIENT, GObject)

#define GSTPOP_CLIENT_DEFAULT_URL "ws://127.0.0.1:9000"

typedef void (*GSTPOPClientResponseCallback) (GSTPOPClient *client,
                                            const gchar *id,
                                            JsonNode *result,
                                            gpointer user_data);

typedef void (*GSTPOPClientErrorCallback) (GSTPOPClient *client,
                                         const gchar *id,
                                         gint code,
                                         const gchar *message,
                                         gpointer user_data);

typedef void (*GSTPOPClientEventCallback) (GSTPOPClient *client,
                                         const gchar *event_type,
                                         JsonNode *data,
                                         gpointer user_data);

typedef void (*GSTPOPClientConnectedCallback) (GSTPOPClient *client,
                                             gboolean success,
                                             const gchar *error_message,
                                             gpointer user_data);

typedef void (*GSTPOPClientClosedCallback) (GSTPOPClient *client,
                                          gpointer user_data);

GSTPOPClient *gstpop_client_new (const gchar *url);
void gstpop_client_free (GSTPOPClient *client);

void gstpop_client_set_response_callback (GSTPOPClient *client,
                                        GSTPOPClientResponseCallback callback,
                                        gpointer user_data);

void gstpop_client_set_error_callback (GSTPOPClient *client,
                                     GSTPOPClientErrorCallback callback,
                                     gpointer user_data);

void gstpop_client_set_event_callback (GSTPOPClient *client,
                                     GSTPOPClientEventCallback callback,
                                     gpointer user_data);

void gstpop_client_set_connected_callback (GSTPOPClient *client,
                                         GSTPOPClientConnectedCallback callback,
                                         gpointer user_data);

void gstpop_client_set_closed_callback (GSTPOPClient *client,
                                      GSTPOPClientClosedCallback callback,
                                      gpointer user_data);

void gstpop_client_connect (GSTPOPClient *client);
void gstpop_client_disconnect (GSTPOPClient *client);
gboolean gstpop_client_is_connected (GSTPOPClient *client);

gchar *gstpop_client_send_request (GSTPOPClient *client,
                                 const gchar *method,
                                 JsonObject *params);

/* Convenience methods */
gchar *gstpop_client_list_pipelines (GSTPOPClient *client);
gchar *gstpop_client_create_pipeline (GSTPOPClient *client, const gchar *description);
gchar *gstpop_client_update_pipeline (GSTPOPClient *client, const gchar *pipeline_id, const gchar *description);
gchar *gstpop_client_remove_pipeline (GSTPOPClient *client, const gchar *pipeline_id);
gchar *gstpop_client_get_pipeline_info (GSTPOPClient *client, const gchar *pipeline_id);
gchar *gstpop_client_play (GSTPOPClient *client, const gchar *pipeline_id);
gchar *gstpop_client_pause (GSTPOPClient *client, const gchar *pipeline_id);
gchar *gstpop_client_stop (GSTPOPClient *client, const gchar *pipeline_id);
gchar *gstpop_client_set_state (GSTPOPClient *client, const gchar *pipeline_id, const gchar *state);
gchar *gstpop_client_snapshot (GSTPOPClient *client, const gchar *pipeline_id, const gchar *details);
gchar *gstpop_client_get_position (GSTPOPClient *client, const gchar *pipeline_id);
gchar *gstpop_client_get_version (GSTPOPClient *client);
gchar *gstpop_client_get_info (GSTPOPClient *client);
gchar *gstpop_client_get_pipeline_count (GSTPOPClient *client);
gchar *gstpop_client_get_elements (GSTPOPClient *client, const gchar *detail);
gchar *gstpop_client_discover_uri (GSTPOPClient *client, const gchar *uri, guint timeout);
gchar *gstpop_client_play_uri (GSTPOPClient *client, const gchar *uri, gboolean use_playbin2);

/* Utility functions */
gchar *gstpop_json_to_pretty_string (JsonNode *node);

G_END_DECLS

#endif /* _GSTPOP_CLIENT_H_ */
