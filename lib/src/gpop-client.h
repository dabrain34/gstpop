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

#ifndef _GPOP_CLIENT_H_
#define _GPOP_CLIENT_H_

#include <glib.h>
#include <gio/gio.h>
#include <libsoup/soup.h>
#include <json-glib/json-glib.h>

G_BEGIN_DECLS

#define GPOP_TYPE_CLIENT (gpop_client_get_type())
G_DECLARE_FINAL_TYPE (GPOPClient, gpop_client, GPOP, CLIENT, GObject)

#define GPOP_CLIENT_DEFAULT_URL "ws://127.0.0.1:9000"

typedef void (*GPOPClientResponseCallback) (GPOPClient *client,
                                            const gchar *id,
                                            JsonNode *result,
                                            gpointer user_data);

typedef void (*GPOPClientErrorCallback) (GPOPClient *client,
                                         const gchar *id,
                                         gint code,
                                         const gchar *message,
                                         gpointer user_data);

typedef void (*GPOPClientEventCallback) (GPOPClient *client,
                                         const gchar *event_type,
                                         JsonNode *data,
                                         gpointer user_data);

typedef void (*GPOPClientConnectedCallback) (GPOPClient *client,
                                             gboolean success,
                                             const gchar *error_message,
                                             gpointer user_data);

typedef void (*GPOPClientClosedCallback) (GPOPClient *client,
                                          gpointer user_data);

GPOPClient *gpop_client_new (const gchar *url);
void gpop_client_free (GPOPClient *client);

void gpop_client_set_response_callback (GPOPClient *client,
                                        GPOPClientResponseCallback callback,
                                        gpointer user_data);

void gpop_client_set_error_callback (GPOPClient *client,
                                     GPOPClientErrorCallback callback,
                                     gpointer user_data);

void gpop_client_set_event_callback (GPOPClient *client,
                                     GPOPClientEventCallback callback,
                                     gpointer user_data);

void gpop_client_set_connected_callback (GPOPClient *client,
                                         GPOPClientConnectedCallback callback,
                                         gpointer user_data);

void gpop_client_set_closed_callback (GPOPClient *client,
                                      GPOPClientClosedCallback callback,
                                      gpointer user_data);

void gpop_client_connect (GPOPClient *client);
void gpop_client_disconnect (GPOPClient *client);
gboolean gpop_client_is_connected (GPOPClient *client);

gchar *gpop_client_send_request (GPOPClient *client,
                                 const gchar *method,
                                 JsonObject *params);

/* Convenience methods */
gchar *gpop_client_list_pipelines (GPOPClient *client);
gchar *gpop_client_create_pipeline (GPOPClient *client, const gchar *description);
gchar *gpop_client_update_pipeline (GPOPClient *client, const gchar *pipeline_id, const gchar *description);
gchar *gpop_client_remove_pipeline (GPOPClient *client, const gchar *pipeline_id);
gchar *gpop_client_get_pipeline_info (GPOPClient *client, const gchar *pipeline_id);
gchar *gpop_client_play (GPOPClient *client, const gchar *pipeline_id);
gchar *gpop_client_pause (GPOPClient *client, const gchar *pipeline_id);
gchar *gpop_client_stop (GPOPClient *client, const gchar *pipeline_id);
gchar *gpop_client_set_state (GPOPClient *client, const gchar *pipeline_id, const gchar *state);
gchar *gpop_client_snapshot (GPOPClient *client, const gchar *pipeline_id, const gchar *details);
gchar *gpop_client_get_position (GPOPClient *client, const gchar *pipeline_id);
gchar *gpop_client_get_version (GPOPClient *client);
gchar *gpop_client_get_info (GPOPClient *client);
gchar *gpop_client_get_pipeline_count (GPOPClient *client);
gchar *gpop_client_get_elements (GPOPClient *client, const gchar *detail);

/* Utility functions */
gchar *gpop_json_to_pretty_string (JsonNode *node);

G_END_DECLS

#endif /* _GPOP_CLIENT_H_ */
