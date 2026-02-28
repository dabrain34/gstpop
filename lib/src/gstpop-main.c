/*
 * GStreamer Prince of Parser
 *
 * Copyright (C) 2020 Stéphane Cerveau <scerveau@gmail.com>
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


#include "gstpop-private.h"

#ifdef G_OS_UNIX
#include <glib-unix.h>
#endif

/* ---------------------------------------------------------------------------------------------------- */

typedef struct _MainApp
{
  GSTPOPManager *manager;
  GMainLoop *loop;
#ifdef G_OS_UNIX
  guint signal_watch_intr_id;
#endif
  gchar **pipeline_desc_array;
} MainApp;

void
quit_app (MainApp * app)
{
  if (app->loop)
    g_main_loop_quit (app->loop);
}

#if defined(G_OS_UNIX)
/* As the interrupt handler is dispatched from GMainContext as a GSourceFunc
 * handler, we can react to this by posting a message. */
static gboolean
intr_handler (gpointer user_data)
{
  MainApp *app = (MainApp *) user_data;

  GSTPOP_LOG ("handling interrupt.");
  quit_app (app);
  /* remove signal handler */
  app->signal_watch_intr_id = 0;

  return G_SOURCE_REMOVE;
}
#endif

static void
on_bus_acquired (GDBusConnection * connection,
    const gchar * name, gpointer user_data)
{
  guint i = 0;
  gchar **pipeline_desc;
  MainApp *app = (MainApp *) user_data;

  GSTPOP_LOG ("Acquired a message bus connection %s", name);

  /* Create a new manager */
  app->manager = gstpop_manager_new (connection);

  /* Add hardcoded edge to the manager */
  for (pipeline_desc = app->pipeline_desc_array;
      pipeline_desc != NULL && *pipeline_desc != NULL; ++pipeline_desc) {
    gstpop_manager_add_pipeline (app->manager, i++, *pipeline_desc, NULL);
  }
}

static void
on_name_acquired (GDBusConnection * connection,
    const gchar * name, gpointer user_data)
{
  GSTPOP_LOG ("Acquired the name %s", name);
}

static void
on_name_lost (GDBusConnection * connection,
    const gchar * name, gpointer user_data)
{
  GSTPOP_LOG ("Lost the name %s", name);
}


gint
gstpop_main (gint argc, gchar * argv[])
{
  int res = 0;
  GError *err = NULL;
  GOptionContext *ctx;
  guint dbus_id = 0;

  MainApp *app = g_new0 (MainApp, 1);

  GOptionEntry options[] = {
    {"pipeline", 'p', 0, G_OPTION_ARG_STRING_ARRAY, &app->pipeline_desc_array,
        "Add pipeline with format ip:port ie 192.168.0.10:5555", NULL}
    ,
    {NULL}
  };

  ctx = g_option_context_new ("[ADDITIONAL ARGUMENTS]");
  g_option_context_add_main_entries (ctx, options, NULL);
  g_option_context_add_group (ctx, gst_init_get_option_group ());
  if (!g_option_context_parse (ctx, &argc, &argv, &err)) {
    GSTPOP_LOG ("Error initializing: %s", err->message);
    res = -1;
    goto done;
  }
  g_option_context_free (ctx);

  app->loop = g_main_loop_new (NULL, FALSE);

  dbus_id = g_bus_own_name (G_BUS_TYPE_SESSION,
      "org.gstpop",
      G_BUS_NAME_OWNER_FLAGS_ALLOW_REPLACEMENT |
      G_BUS_NAME_OWNER_FLAGS_REPLACE,
      on_bus_acquired, on_name_acquired, on_name_lost, app, NULL);

#ifdef G_OS_UNIX
  app->signal_watch_intr_id =
      g_unix_signal_add (SIGINT, (GSourceFunc) intr_handler, app);
#endif
  g_main_loop_run (app->loop);

done:
  if (dbus_id)
    g_bus_unown_name (dbus_id);
  if (app->loop)
    g_main_loop_unref (app->loop);
  gstpop_manager_free (app->manager);
  g_strfreev (app->pipeline_desc_array);

  g_free (app);

  return res;
}
