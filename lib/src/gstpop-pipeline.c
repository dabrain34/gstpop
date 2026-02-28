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

G_DEFINE_TYPE (GSTPOPPipeline, gstpop_pipeline, GSTPOP_TYPE_DBUS_INTERFACE);
#define parent_class gstpop_pipeline_parent_class

#define GSTPOP_PIPELINE_OBJECT_PATH "/org/gstpop/Pipeline%d"

const char gstpop_pipeline_xml_introspection[] =
    "<?xml version='1.0' encoding='UTF-8' ?>"
    "<node>"
    "    <interface name='org.gstpop.GSTPOPInterface'>"
    "       <property name='parser_desc' type='s' access='read'/>"
    "       <property name='id' type='s' access='read'/>"
    "       <property name='streaming' type='b' access='read'/>"
    "    </interface>" "</node>";


static void
gstpop_pipeline_dbus_method_call (GDBusConnection * connection,
    const gchar * sender,
    const gchar * object_path,
    const gchar * interface_name,
    const gchar * method_name,
    GVariant * parameters,
    GDBusMethodInvocation * invocation, gpointer user_data)
{
}

GVariant *
gstpop_pipeline_dbus_get_property (GDBusConnection * connection,
    const gchar * sender,
    const gchar * object_path,
    const gchar * interface_name,
    const gchar * property_name, GError ** error, gpointer user_data)
{
  GVariant *ret = NULL;
  GSTPOPPipeline *pipeline = (GSTPOPPipeline *) user_data;
  if (!g_strcmp0 (property_name, "parser_desc")) {
    ret = g_variant_new ("s", pipeline->parser_desc);
  } else if (!g_strcmp0 (property_name, "id")) {
    ret = g_variant_new ("s", pipeline->id);
  } else if (!g_strcmp0 (property_name, "streaming")) {
    ret = g_variant_new ("b", gstpop_parser_is_playing (pipeline->parser));
  }
  return ret;
}

static gboolean
gstpop_pipeline_dbus_set_property (GDBusConnection * connection,
    const gchar * sender,
    const gchar * object_path,
    const gchar * interface_name,
    const gchar * property_name,
    GVariant * value, GError ** error, gpointer user_data)
{
  /* All properties are read-only */
  g_set_error (error, G_DBUS_ERROR, G_DBUS_ERROR_PROPERTY_READ_ONLY,
      "Property '%s' is read-only", property_name);
  return FALSE;
}

static void
_gstpop_pipeline_clear_desc (GSTPOPPipeline * pipeline)
{
  g_clear_pointer (&pipeline->parser_desc, g_free);
}

static void
gstpop_pipeline_dispose (GObject * object)
{
  GSTPOPPipeline *pipeline = GSTPOP_PIPELINE (object);

  _gstpop_pipeline_clear_desc (pipeline);
  g_clear_pointer (&pipeline->id, g_free);
  g_clear_object (&pipeline->manager);
  g_clear_object (&pipeline->parser);

  if (G_OBJECT_CLASS (parent_class)->dispose)
    G_OBJECT_CLASS (parent_class)->dispose (object);
}

static void
gstpop_pipeline_class_init (GSTPOPPipelineClass * klass)
{
  GSTPOPDBusInterfaceClass *d_klass;
  GObjectClass *gobject_class;

  parent_class = g_type_class_peek_parent (klass);

  gobject_class = G_OBJECT_CLASS (klass);
  gobject_class->dispose = gstpop_pipeline_dispose;

  d_klass = GSTPOP_DBUS_INTERFACE_CLASS (klass);
  d_klass->method_call = gstpop_pipeline_dbus_method_call;
  d_klass->get_property = gstpop_pipeline_dbus_get_property;
  d_klass->set_property = gstpop_pipeline_dbus_set_property;
}

static void
gstpop_pipeline_init (GSTPOPPipeline * pipeline)
{
}

static void
on_stream_state (GSTPOPParser * parser, GSTPOPParserState state,
    gpointer user_data)
{
  GSTPOP_LOG ("state %d", state);

  if (state >= GSTPOP_PARSER_EOS) {
    gstpop_parser_quit (parser);
  }
}

/* Public API */

GSTPOPPipeline *
gstpop_pipeline_new (GSTPOPManager * manager, GDBusConnection * connection,
    guint num)
{
  GSTPOPPipeline *pipeline = g_object_new (GSTPOP_TYPE_PIPELINE, NULL);
  gchar *object_path = g_strdup_printf (GSTPOP_PIPELINE_OBJECT_PATH, num);
  pipeline->manager = g_object_ref (manager);

  if (!gstpop_dbus_interface_register (GSTPOP_DBUS_INTERFACE (pipeline),
          object_path, gstpop_pipeline_xml_introspection, connection)) {
      g_object_unref (pipeline);
      g_free (object_path);
      return NULL;
  }

  pipeline->parser = gstpop_parser_new ();
  g_signal_connect (pipeline->parser, "state-changed",
      G_CALLBACK (on_stream_state), pipeline);

  g_free (object_path);
  return pipeline;
}

void
gstpop_pipeline_free (GSTPOPPipeline * pipeline)
{
  g_clear_object (&pipeline);
}

gboolean
gstpop_pipeline_set_parser_desc (GSTPOPPipeline * pipeline, const gchar * parser_desc)
{
  _gstpop_pipeline_clear_desc (pipeline);

  pipeline->parser_desc = g_strdup (parser_desc);
  return gstpop_parser_play (pipeline->parser, pipeline->parser_desc);
}

gboolean
gstpop_pipeline_set_state (GSTPOPPipeline * pipeline, GSTPOPParserState state)
{
  g_assert (pipeline);

  return gstpop_parser_change_state (pipeline->parser, state);
}
