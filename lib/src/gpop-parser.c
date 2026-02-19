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

#include "gpop-private.h"

struct _GPOPParser
{
  GObject base;
  GstElement *pipeline;
  GstBus *bus;
  GstState state;
  gboolean eos;
  gboolean buffering;

};

G_DEFINE_TYPE (GPOPParser, gpop_parser, G_TYPE_OBJECT);

GST_DEBUG_CATEGORY (gpop_debug);
#define GST_CAT_DEFAULT gpop_debug

enum
{
  SIGNAL_GPOP_PARSER_STATE,
  SIGNAL_LAST
};

static guint gpop_parser_signals[SIGNAL_LAST] = { 0 };

static void gpop_parser_destroy (GPOPParser * parser);

static void
handle_message_application (GPOPParser * parser, const GstStructure * structure)
{
  if (!g_strcmp0 (gst_structure_get_name (structure), "quit-parser"))
    gpop_parser_destroy (parser);
}

static gboolean
message_cb (GstBus * bus, GstMessage * message, gpointer user_data)
{
  GPOPParser *parser = (GPOPParser *) user_data;
  GST_DEBUG_OBJECT (parser, "Received new message %s from %s",
      GST_MESSAGE_TYPE_NAME (message), GST_OBJECT_NAME (message->src));
  switch (GST_MESSAGE_TYPE (message)) {
    case GST_MESSAGE_ERROR:{
      GError *err = NULL;
      gchar *name, *debug = NULL;

      name = gst_object_get_path_string (message->src);
      gst_message_parse_error (message, &err, &debug);

      GST_ERROR_OBJECT (parser, "ERROR: from element %s: %s\n", name,
          err->message);
      if (debug != NULL)
        GST_ERROR_OBJECT (parser, "Additional debug info:%s", debug);

      g_signal_emit (parser,
          gpop_parser_signals[SIGNAL_GPOP_PARSER_STATE], 0, GPOP_PARSER_ERROR);

      g_error_free (err);
      g_free (debug);
      g_free (name);

      break;
    }
    case GST_MESSAGE_WARNING:{
      GError *err = NULL;
      gchar *name, *debug = NULL;

      name = gst_object_get_path_string (message->src);
      gst_message_parse_warning (message, &err, &debug);

      GST_WARNING_OBJECT (parser, "ERROR: from element %s: %s\n", name,
          err->message);
      if (debug != NULL)
        GST_WARNING_OBJECT (parser, "Additional debug info:\n%s\n", debug);

      g_error_free (err);
      g_free (debug);
      g_free (name);
      break;
    }
    case GST_MESSAGE_EOS:
      parser->eos = TRUE;
      g_signal_emit (parser,
          gpop_parser_signals[SIGNAL_GPOP_PARSER_STATE], 0, GPOP_PARSER_EOS);
      break;

    case GST_MESSAGE_STATE_CHANGED:
    {
      GstState old, new, pending;
      if (GST_MESSAGE_SRC (message) == GST_OBJECT_CAST (parser->pipeline)) {
        gst_message_parse_state_changed (message, &old, &new, &pending);
        parser->state = new;
        if (parser->state == GST_STATE_PLAYING)
          g_signal_emit (parser,
              gpop_parser_signals[SIGNAL_GPOP_PARSER_STATE], 0,
              GPOP_PARSER_PLAYING);
      }
      break;
    }
    case GST_MESSAGE_BUFFERING:{
      gint percent;

      gst_message_parse_buffering (message, &percent);
      GST_INFO_OBJECT (parser, "buffering  %d%% ", percent);

      if (percent == 100) {
        /* a 100% message means buffering is done */
        parser->buffering = FALSE;
        /* if the desired state is playing, go back */
        if (parser->state == GST_STATE_PLAYING) {
          GST_INFO_OBJECT (parser,
              "Done buffering, setting pipeline to PLAYING ...");
          gst_element_set_state (parser->pipeline, GST_STATE_PLAYING);
        }
      } else {
        /* buffering busy */
        if (!parser->buffering && parser->state == GST_STATE_PLAYING) {
          /* we were not buffering but PLAYING, PAUSE  the pipeline. */
          GST_INFO_OBJECT (parser, "Buffering, setting pipeline to PAUSED ...");
          gst_element_set_state (parser->pipeline, GST_STATE_PAUSED);
        }
        parser->buffering = TRUE;
      }
      break;
    }
    case GST_MESSAGE_APPLICATION:{
      const GstStructure *s = gst_message_get_structure (message);
      handle_message_application (parser, s);
      break;
    }
    default:
      break;
  }

  return TRUE;
}

static gboolean
gpop_parser_set_player_state (GPOPParser * parser, GstState state)
{
  gboolean res = TRUE;
  GstStateChangeReturn ret;

  g_return_val_if_fail (GPOP_IS_PARSER (parser), FALSE);

  ret = gst_element_set_state (parser->pipeline, state);

  switch (ret) {
    case GST_STATE_CHANGE_FAILURE:
      GST_INFO_OBJECT (parser, "ERROR: %s doesn't want to pause.",
          GST_ELEMENT_NAME (parser->pipeline));
      res = FALSE;
      break;
    case GST_STATE_CHANGE_NO_PREROLL:
      GST_INFO_OBJECT (parser, "%s is live and does not need PREROLL ...",
          GST_ELEMENT_NAME (parser->pipeline));
      break;
    case GST_STATE_CHANGE_ASYNC:
      GST_INFO_OBJECT (parser, "%s is PREROLLING ...",
          GST_ELEMENT_NAME (parser->pipeline));
      break;
    /* fallthrough */
    case GST_STATE_CHANGE_SUCCESS:
      if (parser->state == GST_STATE_PAUSED)
        GST_INFO_OBJECT (parser, "%s is PREROLLED ...",
            GST_ELEMENT_NAME (parser->pipeline));
      break;
  }
  return res;
}

static void
gpop_parser_destroy (GPOPParser * parser)
{
  GST_INFO_OBJECT (parser, "About to destroy the parser");
  if (parser->bus) {
    gst_bus_remove_signal_watch (parser->bus);
    g_clear_object (&parser->bus);
  }
  if (parser->pipeline) {
    gpop_parser_set_player_state (parser, GST_STATE_NULL);
    g_object_unref (parser->pipeline);
    parser->pipeline = NULL;
    GST_INFO_OBJECT (parser, "pipeline destroyed");
  }
}

static void
gpop_parser_dispose (GObject * object)
{
  GPOPParser *parser = GPOP_PARSER (object);
  gpop_parser_destroy (parser);
  g_clear_object (&parser->bus);

  G_OBJECT_CLASS (gpop_parser_parent_class)->dispose (object);
}

static void
gpop_parser_class_init (GPOPParserClass * klass)
{
  GObjectClass *gobject_class;

  gobject_class = G_OBJECT_CLASS (klass);
  gobject_class->dispose = gpop_parser_dispose;

  gpop_parser_signals[SIGNAL_GPOP_PARSER_STATE] =
      g_signal_new ("state-changed", G_TYPE_FROM_CLASS (klass),
      G_SIGNAL_RUN_LAST, G_STRUCT_OFFSET (GPOPParserClass,
          state_changed), NULL, NULL, g_cclosure_marshal_VOID__INT,
      G_TYPE_NONE, 1, G_TYPE_INT);

  GST_DEBUG_CATEGORY_INIT (gpop_debug, "gpop", 0, "gpop-parser");
}

static void
gpop_parser_init (GPOPParser * parser)
{
}

GPOPParser *
gpop_parser_new ()
{
  return g_object_new (GPOP_TYPE_PARSER, NULL);
}

void
gpop_parser_free (GPOPParser * parser)
{
  if (parser)
    gpop_parser_quit (parser);
  g_clear_object (&parser);
}

/* API */
gboolean
gpop_parser_create (GPOPParser * parser, const gchar * parser_desc)
{
  GstElement *parsed_element;
  GstBus *bus;
  GError *err = NULL;
  gchar *desc;

  gpop_parser_destroy (parser);

  if (g_getenv ("GPOP_PIPELINE"))
    desc = g_strdup (g_getenv ("GPOP_PIPELINE"));
  else
    desc = g_strdup (parser_desc);
  GST_INFO_OBJECT (parser, "About to instantiate the parser pipeline '%s'",
      parser_desc);
  parser->state = GST_STATE_NULL;
  parser->pipeline = gst_pipeline_new (NULL);
  parsed_element =
      gst_parse_launch_full (desc, NULL, GST_PARSE_FLAG_NONE, &err);
  g_free (desc);
  if (err) {
    GST_ERROR_OBJECT (parser,
        "Unable to instantiate the pipeline with message '%s'", err->message);
    g_error_free (err);
    /* Clean up the pipeline allocated above to avoid a leak */
    g_clear_object (&parser->pipeline);
    return FALSE;
  }

  gst_bin_add (GST_BIN (parser->pipeline), parsed_element);

  bus = gst_pipeline_get_bus (GST_PIPELINE (parser->pipeline));
  g_signal_connect (G_OBJECT (bus), "message", G_CALLBACK (message_cb), parser);
  gst_bus_add_signal_watch (bus);
  /* Store bus in struct so we can remove the signal watch in destroy */
  parser->bus = bus;

  return TRUE;
}

/* Public APÏ */

gboolean
gpop_parser_play (GPOPParser * parser, const gchar * parser_desc)
{
  if (!gpop_parser_create (parser, parser_desc))
    return FALSE;

  return gpop_parser_set_player_state (parser, GST_STATE_PLAYING);
}

void
gpop_parser_quit (GPOPParser * parser)
{
  gpop_parser_set_player_state (parser, GST_STATE_NULL);
}

gboolean
gpop_parser_is_playing (GPOPParser * parser)
{
  return (parser->state == GST_STATE_PLAYING);
}

gboolean
gpop_parser_change_state (GPOPParser * parser, GPOPParserState state)
{
  gboolean ret = FALSE;
  switch (state) {
    case GPOP_PARSER_READY:
      ret = gpop_parser_set_player_state (parser, GST_STATE_READY);
      break;
    case GPOP_PARSER_PAUSED:
      ret = gpop_parser_set_player_state (parser, GST_STATE_PAUSED);
      break;
    case GPOP_PARSER_PLAYING:
      ret = gpop_parser_set_player_state (parser, GST_STATE_PLAYING);
      break;
    default:
      GST_INFO_OBJECT (parser, "State not supported %d", state);
      break;
  }
  return ret;
}
