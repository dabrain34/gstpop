/*
 * GStreamer Prince of Parser - C WebSocket Client
 *
 * Copyright (C) 2020-2024 Stephane Cerveau <scerveau@igalia.com>
 *
 * SPDX-License-Identifier: GPL-3.0-only
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, version 3 of the License.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

#include <gpop-client.h>
#include <stdio.h>
#include <string.h>
#include <readline/readline.h>
#include <readline/history.h>

typedef struct {
    GPOPClient *client;
    GMainLoop *loop;
} AppContext;

static AppContext *app = NULL;

static void
print_help (void)
{
    g_print ("\nAvailable commands:\n");
    g_print ("  list                      - List all pipelines\n");
    g_print ("  create <description>      - Create a new pipeline\n");
    g_print ("  update <id> <description> - Update pipeline description\n");
    g_print ("  remove <id>               - Remove a pipeline\n");
    g_print ("  info <id>                 - Get pipeline info\n");
    g_print ("  play [id]                 - Play a pipeline\n");
    g_print ("  pause [id]                - Pause a pipeline\n");
    g_print ("  stop [id]                 - Stop a pipeline\n");
    g_print ("  state <id> <state>        - Set pipeline state\n");
    g_print ("  snapshot <id> [details]   - Get DOT graph (details: media, caps, states, all)\n");
    g_print ("  position [id]             - Get pipeline position/duration\n");
    g_print ("  version                   - Get daemon version\n");
    g_print ("  sysinfo                   - Get daemon and GStreamer info\n");
    g_print ("  count                     - Get pipeline count\n");
    g_print ("  elements [detail]         - List GStreamer elements (detail: none, summary, full)\n");
    g_print ("  help                      - Show this help\n");
    g_print ("  quit                      - Exit\n");
    g_print ("\n");
}

static gchar *
strip_quotes (const gchar *str)
{
    gsize len = strlen (str);
    if (len >= 2) {
        if ((str[0] == '"' && str[len - 1] == '"') ||
            (str[0] == '\'' && str[len - 1] == '\'')) {
            return g_strndup (str + 1, len - 2);
        }
    }
    return g_strdup (str);
}

static void
on_response (GPOPClient *client,
             const gchar *id,
             JsonNode *result,
             gpointer user_data)
{
    (void) client;
    (void) user_data;

    gchar *result_str = gpop_json_to_pretty_string (result);
    g_print ("\n[RESPONSE] id=%s: %s\n", id, result_str);
    rl_forced_update_display ();
    g_free (result_str);
}

static void
on_error (GPOPClient *client,
          const gchar *id,
          gint code,
          const gchar *message,
          gpointer user_data)
{
    (void) client;
    (void) user_data;

    g_print ("\n[ERROR] id=%s: %s (code: %d)\n", id, message, code);
    rl_forced_update_display ();
}

static void
on_event (GPOPClient *client,
          const gchar *event_type,
          JsonNode *data,
          gpointer user_data)
{
    (void) client;
    (void) user_data;

    gchar *data_str = gpop_json_to_pretty_string (data);
    g_print ("\n[EVENT] %s: %s\n", event_type, data_str);
    rl_forced_update_display ();
    g_free (data_str);
}

static void
on_closed (GPOPClient *client, gpointer user_data)
{
    AppContext *ctx = (AppContext *) user_data;
    (void) client;

    g_print ("\nConnection closed\n");
    g_main_loop_quit (ctx->loop);
}

static gboolean
process_command (AppContext *ctx, const gchar *line)
{
    gchar **parts = g_strsplit (line, " ", -1);
    gint argc = g_strv_length (parts);
    gchar *request_id = NULL;

    if (argc == 0 || strlen (parts[0]) == 0) {
        g_strfreev (parts);
        return TRUE;
    }

    const gchar *cmd = parts[0];

    if (g_strcmp0 (cmd, "list") == 0) {
        request_id = gpop_client_list_pipelines (ctx->client);
    }
    else if (g_strcmp0 (cmd, "create") == 0 && argc > 1) {
        gchar *joined = g_strjoinv (" ", parts + 1);
        gchar *description = strip_quotes (joined);
        request_id = gpop_client_create_pipeline (ctx->client, description);
        g_free (description);
        g_free (joined);
    }
    else if (g_strcmp0 (cmd, "update") == 0 && argc > 2) {
        gchar *joined = g_strjoinv (" ", parts + 2);
        gchar *description = strip_quotes (joined);
        request_id = gpop_client_update_pipeline (ctx->client, parts[1], description);
        g_free (description);
        g_free (joined);
    }
    else if (g_strcmp0 (cmd, "remove") == 0 && argc == 2) {
        request_id = gpop_client_remove_pipeline (ctx->client, parts[1]);
    }
    else if (g_strcmp0 (cmd, "info") == 0 && argc == 2) {
        request_id = gpop_client_get_pipeline_info (ctx->client, parts[1]);
    }
    else if (g_strcmp0 (cmd, "play") == 0) {
        const gchar *pipeline_id = (argc >= 2) ? parts[1] : NULL;
        request_id = gpop_client_play (ctx->client, pipeline_id);
    }
    else if (g_strcmp0 (cmd, "pause") == 0) {
        const gchar *pipeline_id = (argc >= 2) ? parts[1] : NULL;
        request_id = gpop_client_pause (ctx->client, pipeline_id);
    }
    else if (g_strcmp0 (cmd, "stop") == 0) {
        const gchar *pipeline_id = (argc >= 2) ? parts[1] : NULL;
        request_id = gpop_client_stop (ctx->client, pipeline_id);
    }
    else if (g_strcmp0 (cmd, "state") == 0 && argc == 3) {
        request_id = gpop_client_set_state (ctx->client, parts[1], parts[2]);
    }
    else if (g_strcmp0 (cmd, "snapshot") == 0 && argc >= 2) {
        const gchar *details = (argc > 2) ? parts[2] : NULL;
        request_id = gpop_client_snapshot (ctx->client, parts[1], details);
    }
    else if (g_strcmp0 (cmd, "position") == 0) {
        const gchar *pipeline_id = (argc > 1) ? parts[1] : NULL;
        request_id = gpop_client_get_position (ctx->client, pipeline_id);
    }
    else if (g_strcmp0 (cmd, "version") == 0) {
        request_id = gpop_client_get_version (ctx->client);
    }
    else if (g_strcmp0 (cmd, "sysinfo") == 0) {
        request_id = gpop_client_get_info (ctx->client);
    }
    else if (g_strcmp0 (cmd, "count") == 0) {
        request_id = gpop_client_get_pipeline_count (ctx->client);
    }
    else if (g_strcmp0 (cmd, "elements") == 0) {
        const gchar *detail = (argc > 1) ? parts[1] : NULL;
        request_id = gpop_client_get_elements (ctx->client, detail);
    }
    else if (g_strcmp0 (cmd, "help") == 0) {
        print_help ();
    }
    else if (g_strcmp0 (cmd, "quit") == 0 || g_strcmp0 (cmd, "exit") == 0) {
        g_strfreev (parts);
        return FALSE;
    }
    else {
        g_print ("Unknown command or missing arguments. Type 'help' for available commands.\n");
    }

    g_free (request_id);
    g_strfreev (parts);
    return TRUE;
}

static void
readline_handler (char *line)
{
    if (line == NULL) {
        /* EOF (Ctrl+D) */
        g_print ("\nGoodbye!\n");
        g_main_loop_quit (app->loop);
        return;
    }

    g_strstrip (line);

    if (strlen (line) > 0) {
        add_history (line);

        if (!process_command (app, line)) {
            g_print ("Goodbye!\n");
            free (line);
            g_main_loop_quit (app->loop);
            return;
        }
    }

    free (line);
}

static gboolean
on_stdin_ready (GIOChannel *source,
                GIOCondition condition,
                gpointer user_data)
{
    (void) source;
    (void) user_data;

    if (condition & G_IO_IN) {
        rl_callback_read_char ();
    }

    if (condition & G_IO_HUP) {
        g_print ("\nGoodbye!\n");
        g_main_loop_quit (app->loop);
        return FALSE;
    }

    return TRUE;
}

static void
on_connected (GPOPClient *client,
              gboolean success,
              const gchar *error_message,
              gpointer user_data)
{
    AppContext *ctx = (AppContext *) user_data;
    (void) client;

    if (!success) {
        g_printerr ("Failed to connect: %s\n", error_message);
        g_main_loop_quit (ctx->loop);
        return;
    }

    g_print ("Connected!\n");
    print_help ();

    /* Setup readline with callback interface for GLib integration */
    rl_callback_handler_install ("> ", readline_handler);

    /* Watch stdin for input */
    GIOChannel *stdin_channel = g_io_channel_unix_new (fileno (stdin));
    g_io_add_watch (stdin_channel, G_IO_IN | G_IO_HUP, on_stdin_ready, ctx);
    g_io_channel_unref (stdin_channel);
}

gint
main (gint argc, gchar *argv[])
{
    const gchar *url = GPOP_CLIENT_DEFAULT_URL;

    if (argc > 1) {
        url = argv[1];
    }

    app = g_new0 (AppContext, 1);
    app->loop = g_main_loop_new (NULL, FALSE);
    app->client = gpop_client_new (url);

    /* Set up callbacks */
    gpop_client_set_response_callback (app->client, on_response, app);
    gpop_client_set_error_callback (app->client, on_error, app);
    gpop_client_set_event_callback (app->client, on_event, app);
    gpop_client_set_connected_callback (app->client, on_connected, app);
    gpop_client_set_closed_callback (app->client, on_closed, app);

    g_print ("Connecting to %s...\n", url);
    gpop_client_connect (app->client);

    g_main_loop_run (app->loop);

    /* Cleanup */
    rl_callback_handler_remove ();
    clear_history ();

    gpop_client_free (app->client);
    g_main_loop_unref (app->loop);
    g_free (app);

    return 0;
}
