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

G_DEFINE_TYPE (GPOPManager, gpop_manager, GPOP_TYPE_DBUS_INTERFACE);
#define parent_class gpop_manager_parent_class

#define GPOP_MANAGER_OBJECT_PATH "/org/gpop/Manager"

const char gpop_manager_xml_introspection[] =
    "<?xml version='1.0' encoding='UTF-8' ?>"
    "<node>"
    "    <interface name='org.gpop.GPOPInterface'>"
    "        <method name='GetPipelineDesc'>"
    "		<arg type='s' name='id' direction='in'/>"
    "		<arg type='s' name='desc' direction='out'/>"
    "        </method>"
    "        <method name='AddPipeline'>"
    "		<arg type='s' name='pipeline_desc' direction='in'/>"
    "        </method>"
    "        <method name='RemovePipeline'>"
    "		<arg type='s' name='id' direction='in'/>"
    "        </method>"
    "       <property name='Pipelines' type='i' access='read'/>"
    "       <property name='Version' type='s' access='read'/>"
    "    </interface>" "</node>";

static guint
gpop_manager_pipelines_count (GPOPManager * manager)
{
  return g_list_length (manager->pipelines);
}

static GPOPPipeline *
gpop_manager_get_pipeline_by_id (GPOPManager * manager, gchar * id)
{
  GList *l;
  GPOPPipeline *pipeline = NULL;

  for (l = manager->pipelines; l != NULL; l = g_list_next (l)) {
    pipeline = (GPOPPipeline *) l->data;
    if (!g_strcmp0 (pipeline->id, id))
      return pipeline;
  }
  return NULL;
}

static void
gpop_manager_dbus_method_call (GDBusConnection * connection,
    const gchar * sender,
    const gchar * object_path,
    const gchar * interface_name,
    const gchar * method_name,
    GVariant * parameters,
    GDBusMethodInvocation * invocation, gpointer user_data)
{
  GPOPManager *manager = (GPOPManager *) user_data;
  GVariant *ret = NULL;
  if (!g_strcmp0 (method_name, "GetPipelineDesc")) {
    gchar *id;
    GPOPPipeline *pipeline;
    g_variant_get (parameters, "(s)", &id);

    pipeline = gpop_manager_get_pipeline_by_id (manager, id);
    g_free (id);
    if (pipeline) {
      ret = g_variant_new ("(s)", pipeline->parser_desc);
    } else
      ret = g_variant_new ("(s)", "");
  } else if (!g_strcmp0 (method_name, "AddPipeline")) {
    gchar *parser_desc;
    g_variant_get (parameters, "(s)", &parser_desc);
    gpop_manager_add_pipeline (manager, gpop_manager_pipelines_count (manager),
        parser_desc, NULL);
    g_free (parser_desc);
  } else if (!g_strcmp0 (method_name, "RemovePipeline")) {
    gchar *id;
    g_variant_get (parameters, "(s)", &id);
    gpop_manager_remove_pipeline (manager, id);
    g_free (id);
  }

  g_dbus_method_invocation_return_value (invocation, ret);
  g_dbus_connection_flush (connection, NULL, NULL, NULL);
}

GVariant *
gpop_manager_dbus_get_property (GDBusConnection * connection,
    const gchar * sender,
    const gchar * object_path,
    const gchar * interface_name,
    const gchar * property_name, GError ** error, gpointer user_data)
{
  GVariant *ret = NULL;
  GPOPManager *manager = (GPOPManager *) user_data;
  if (!g_strcmp0 (property_name, "Pipelines")) {
    ret = g_variant_new ("i", g_list_length (manager->pipelines));
  } else if (!g_strcmp0 (property_name, "Version")) {
    ret = g_variant_new ("s", "0.0.1");
  }
  return ret;
}

static gboolean
gpop_manager_dbus_set_property (GDBusConnection * connection,
    const gchar * sender,
    const gchar * object_path,
    const gchar * interface_name,
    const gchar * property_name,
    GVariant * value, GError ** error, gpointer user_data)
{
  g_set_error (error, G_DBUS_ERROR, G_DBUS_ERROR_PROPERTY_READ_ONLY,
      "Property '%s' is read-only", property_name);
  return FALSE;
}

static void
gpop_manager_dispose (GObject * object)
{

  GPOPManager *manager = GPOP_MANAGER (object);

  g_list_free_full (manager->pipelines, (GDestroyNotify) gpop_pipeline_free);

  if (G_OBJECT_CLASS (parent_class)->dispose)
    G_OBJECT_CLASS (parent_class)->dispose (object);
}

static void
gpop_manager_class_init (GPOPManagerClass * klass)
{
  GPOPDBusInterfaceClass *d_klass;
  GObjectClass *gobject_class;

  parent_class = g_type_class_peek_parent (klass);

  gobject_class = G_OBJECT_CLASS (klass);
  gobject_class->dispose = gpop_manager_dispose;

  d_klass = GPOP_DBUS_INTERFACE_CLASS (klass);
  d_klass->method_call = gpop_manager_dbus_method_call;
  d_klass->get_property = gpop_manager_dbus_get_property;
  d_klass->set_property = gpop_manager_dbus_set_property;
}

static void
gpop_manager_init (GPOPManager * manager)
{
}

GPOPManager *
gpop_manager_new (GDBusConnection * connection)
{
  GPOPManager *manager = g_object_new (GPOP_TYPE_MANAGER, NULL);
  if (gpop_dbus_interface_register (GPOP_DBUS_INTERFACE (manager),
          GPOP_MANAGER_OBJECT_PATH, gpop_manager_xml_introspection, connection))
    return manager;
  else {
    g_object_unref (manager);
    return NULL;
  }
}

void
gpop_manager_free (GPOPManager * manager)
{
  g_clear_object (&manager);
}

void
gpop_manager_add_pipeline (GPOPManager * manager, guint num, const gchar * parser_desc, gchar* id)
{
  GPOPPipeline *pipeline =
      gpop_pipeline_new (manager, manager->base.connection, num);

  if (id)
    pipeline->id = g_strdup (id);
  else
    pipeline->id = g_strdup_printf ("pipeline_%u", num);

  if (gpop_pipeline_set_parser_desc (pipeline, parser_desc)) {
    GPOP_LOG
        ("An pipeline with id '%s' has been created successfully for description '%s'",
        pipeline->id, parser_desc);
    manager->pipelines = g_list_append (manager->pipelines, pipeline);
  } else {
    GPOP_LOG ("Unable to add the pipeline with description %s", parser_desc);
    gpop_pipeline_free (pipeline);
  }
}

void
gpop_manager_remove_pipeline (GPOPManager * manager, gchar* id)
{
  GPOPPipeline *pipeline = gpop_manager_get_pipeline_by_id (manager, id);
  if (!pipeline) {
    GPOP_LOG ("pipeline with id %s does not exist", id);
    return;
  }
  manager->pipelines = g_list_remove (manager->pipelines, pipeline);
  gpop_pipeline_free (pipeline);
}
