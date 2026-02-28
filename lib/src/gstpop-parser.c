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

struct _GSTPOPParser
{
  GObject base;
  GstElement *pipeline;
  GstBus *bus;
  GstState state;
  gboolean eos;
  gboolean buffering;

};

G_DEFINE_TYPE (GSTPOPParser, gstpop_parser, G_TYPE_OBJECT);

GST_DEBUG_CATEGORY (gstpop_debug);
#define GST_CAT_DEFAULT gstpop_debug

enum
{
  SIGNAL_GSTPOP_PARSER_STATE,
  SIGNAL_LAST
};

static guint gstpop_parser_signals[SIGNAL_LAST] = { 0 };

static void gstpop_parser_destroy (GSTPOPParser * parser);

static void
handle_message_application (GSTPOPParser * parser, const GstStructure * structure)
{
  if (!g_strcmp0 (gst_structure_get_name (structure), "quit-parser"))
    gstpop_parser_destroy (parser);
}

static gboolean
message_cb (GstBus * bus, GstMessage * message, gpointer user_data)
{
  GSTPOPParser *parser = (GSTPOPParser *) user_data;
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
          gstpop_parser_signals[SIGNAL_GSTPOP_PARSER_STATE], 0, GSTPOP_PARSER_ERROR);

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
          gstpop_parser_signals[SIGNAL_GSTPOP_PARSER_STATE], 0, GSTPOP_PARSER_EOS);
      break;

    case GST_MESSAGE_STATE_CHANGED:
    {
      GstState old, new, pending;
      if (GST_MESSAGE_SRC (message) == GST_OBJECT_CAST (parser->pipeline)) {
        gst_message_parse_state_changed (message, &old, &new, &pending);
        parser->state = new;
        if (parser->state == GST_STATE_PLAYING)
          g_signal_emit (parser,
              gstpop_parser_signals[SIGNAL_GSTPOP_PARSER_STATE], 0,
              GSTPOP_PARSER_PLAYING);
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
gstpop_parser_set_player_state (GSTPOPParser * parser, GstState state)
{
  gboolean res = TRUE;
  GstStateChangeReturn ret;

  g_return_val_if_fail (GSTPOP_IS_PARSER (parser), FALSE);

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
gstpop_parser_destroy (GSTPOPParser * parser)
{
  GST_INFO_OBJECT (parser, "About to destroy the parser");
  if (parser->bus) {
    gst_bus_remove_signal_watch (parser->bus);
    g_clear_object (&parser->bus);
  }
  if (parser->pipeline) {
    gstpop_parser_set_player_state (parser, GST_STATE_NULL);
    g_object_unref (parser->pipeline);
    parser->pipeline = NULL;
    GST_INFO_OBJECT (parser, "pipeline destroyed");
  }
}

static void
gstpop_parser_dispose (GObject * object)
{
  GSTPOPParser *parser = GSTPOP_PARSER (object);
  gstpop_parser_destroy (parser);
  g_clear_object (&parser->bus);

  G_OBJECT_CLASS (gstpop_parser_parent_class)->dispose (object);
}

static void
gstpop_parser_class_init (GSTPOPParserClass * klass)
{
  GObjectClass *gobject_class;

  gobject_class = G_OBJECT_CLASS (klass);
  gobject_class->dispose = gstpop_parser_dispose;

  gstpop_parser_signals[SIGNAL_GSTPOP_PARSER_STATE] =
      g_signal_new ("state-changed", G_TYPE_FROM_CLASS (klass),
      G_SIGNAL_RUN_LAST, G_STRUCT_OFFSET (GSTPOPParserClass,
          state_changed), NULL, NULL, g_cclosure_marshal_VOID__INT,
      G_TYPE_NONE, 1, G_TYPE_INT);

  GST_DEBUG_CATEGORY_INIT (gstpop_debug, "gstpop", 0, "gstpop-parser");
}

static void
gstpop_parser_init (GSTPOPParser * parser)
{
}

GSTPOPParser *
gstpop_parser_new ()
{
  return g_object_new (GSTPOP_TYPE_PARSER, NULL);
}

void
gstpop_parser_free (GSTPOPParser * parser)
{
  if (parser)
    gstpop_parser_quit (parser);
  g_clear_object (&parser);
}

/* API */
gboolean
gstpop_parser_create (GSTPOPParser * parser, const gchar * parser_desc)
{
  GstElement *parsed_element;
  GstBus *bus;
  GError *err = NULL;
  gchar *desc;

  gstpop_parser_destroy (parser);

  if (g_getenv ("GSTPOP_PIPELINE"))
    desc = g_strdup (g_getenv ("GSTPOP_PIPELINE"));
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
gstpop_parser_play (GSTPOPParser * parser, const gchar * parser_desc)
{
  if (!gstpop_parser_create (parser, parser_desc))
    return FALSE;

  return gstpop_parser_set_player_state (parser, GST_STATE_PLAYING);
}

void
gstpop_parser_quit (GSTPOPParser * parser)
{
  gstpop_parser_set_player_state (parser, GST_STATE_NULL);
}

gboolean
gstpop_parser_is_playing (GSTPOPParser * parser)
{
  return (parser->state == GST_STATE_PLAYING);
}

gboolean
gstpop_parser_change_state (GSTPOPParser * parser, GSTPOPParserState state)
{
  gboolean ret = FALSE;
  switch (state) {
    case GSTPOP_PARSER_READY:
      ret = gstpop_parser_set_player_state (parser, GST_STATE_READY);
      break;
    case GSTPOP_PARSER_PAUSED:
      ret = gstpop_parser_set_player_state (parser, GST_STATE_PAUSED);
      break;
    case GSTPOP_PARSER_PLAYING:
      ret = gstpop_parser_set_player_state (parser, GST_STATE_PLAYING);
      break;
    default:
      GST_INFO_OBJECT (parser, "State not supported %d", state);
      break;
  }
  return ret;
}
