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

G_DEFINE_TYPE (GPOPPipeline, gpop_pipeline, GPOP_TYPE_DBUS_INTERFACE);
#define parent_class gpop_pipeline_parent_class

#define GPOP_PIPELINE_OBJECT_PATH "/org/gpop/Pipeline%d"

const char gpop_pipeline_xml_introspection[] =
    "<?xml version='1.0' encoding='UTF-8' ?>"
    "<node>"
    "    <interface name='org.gpop.GPOPInterface'>"
    "       <property name='parser_desc' type='s' access='read'/>"
    "       <property name='id' type='s' access='read'/>"
    "       <property name='streaming' type='b' access='read'/>"
    "    </interface>" "</node>";


static void
gpop_pipeline_dbus_method_call (GDBusConnection * connection,
    const gchar * sender,
    const gchar * object_path,
    const gchar * interface_name,
    const gchar * method_name,
    GVariant * parameters,
    GDBusMethodInvocation * invocation, gpointer user_data)
{
}

GVariant *
gpop_pipeline_dbus_get_property (GDBusConnection * connection,
    const gchar * sender,
    const gchar * object_path,
    const gchar * interface_name,
    const gchar * property_name, GError ** error, gpointer user_data)
{
  GVariant *ret = NULL;
  GPOPPipeline *pipeline = (GPOPPipeline *) user_data;
  if (!g_strcmp0 (property_name, "parser_desc")) {
    ret = g_variant_new ("s", pipeline->parser_desc);
  } else if (!g_strcmp0 (property_name, "id")) {
    ret = g_variant_new ("s", pipeline->id);
  } else if (!g_strcmp0 (property_name, "streaming")) {
    ret = g_variant_new ("b", gpop_parser_is_playing (pipeline->parser));
  }
  return ret;
}

static gboolean
gpop_pipeline_dbus_set_property (GDBusConnection * connection,
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
_gpop_pipeline_clear_desc (GPOPPipeline * pipeline)
{
  g_clear_pointer (&pipeline->parser_desc, g_free);
}

static void
gpop_pipeline_dispose (GObject * object)
{
  GPOPPipeline *pipeline = GPOP_PIPELINE (object);

  _gpop_pipeline_clear_desc (pipeline);
  g_clear_pointer (&pipeline->id, g_free);
  g_clear_object (&pipeline->manager);
  g_clear_object (&pipeline->parser);

  if (G_OBJECT_CLASS (parent_class)->dispose)
    G_OBJECT_CLASS (parent_class)->dispose (object);
}

static void
gpop_pipeline_class_init (GPOPPipelineClass * klass)
{
  GPOPDBusInterfaceClass *d_klass;
  GObjectClass *gobject_class;

  parent_class = g_type_class_peek_parent (klass);

  gobject_class = G_OBJECT_CLASS (klass);
  gobject_class->dispose = gpop_pipeline_dispose;

  d_klass = GPOP_DBUS_INTERFACE_CLASS (klass);
  d_klass->method_call = gpop_pipeline_dbus_method_call;
  d_klass->get_property = gpop_pipeline_dbus_get_property;
  d_klass->set_property = gpop_pipeline_dbus_set_property;
}

static void
gpop_pipeline_init (GPOPPipeline * pipeline)
{
}

static void
on_stream_state (GPOPParser * parser, GPOPParserState state,
    gpointer user_data)
{
  GPOP_LOG ("state %d", state);

  if (state >= GPOP_PARSER_EOS) {
    gpop_parser_quit (parser);
  }
}

/* Public API */

GPOPPipeline *
gpop_pipeline_new (GPOPManager * manager, GDBusConnection * connection,
    guint num)
{
  GPOPPipeline *pipeline = g_object_new (GPOP_TYPE_PIPELINE, NULL);
  gchar *object_path = g_strdup_printf (GPOP_PIPELINE_OBJECT_PATH, num);
  pipeline->manager = g_object_ref (manager);

  if (!gpop_dbus_interface_register (GPOP_DBUS_INTERFACE (pipeline),
          object_path, gpop_pipeline_xml_introspection, connection)) {
      g_object_unref (pipeline);
      g_free (object_path);
      return NULL;
  }

  pipeline->parser = gpop_parser_new ();
  g_signal_connect (pipeline->parser, "state-changed",
      G_CALLBACK (on_stream_state), pipeline);

  g_free (object_path);
  return pipeline;
}

void
gpop_pipeline_free (GPOPPipeline * pipeline)
{
  g_clear_object (&pipeline);
}

gboolean
gpop_pipeline_set_parser_desc (GPOPPipeline * pipeline, const gchar * parser_desc)
{
  _gpop_pipeline_clear_desc (pipeline);

  pipeline->parser_desc = g_strdup (parser_desc);
  return gpop_parser_play (pipeline->parser, pipeline->parser_desc);
}

gboolean
gpop_pipeline_set_state (GPOPPipeline * pipeline, GPOPParserState state)
{
  g_assert (pipeline);

  return gpop_parser_change_state (pipeline->parser, state);
}
