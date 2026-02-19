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

#ifndef _GPOP_DBUS_INTERFACE_H_
#define _GPOP_DBUS_INTERFACE_H_

#include <gio/gio.h>
#include <glib.h>

#define GPOP_TYPE_DBUS_INTERFACE    (gpop_dbus_interface_get_type())
#define GPOP_DBUS_INTERFACE(obj)            (G_TYPE_CHECK_INSTANCE_CAST((obj),\
                                              GPOP_TYPE_DBUS_INTERFACE, GPOPDBusInterface))
#define GPOP_DBUS_INTERFACE_CLASS(klass)    (G_TYPE_CHECK_CLASS_CAST((klass),\
                                              GPOP_TYPE_DBUS_INTERFACE, GPOPDBusInterfaceClass))
#define GPOP_DBUS_INTERFACE_GET_CLASS(obj)  (G_TYPE_INSTANCE_GET_CLASS ((obj),\
                                              GPOP_TYPE_DBUS_INTERFACE, GPOPDBusInterfaceClass))
#define GPOP_IS_DBUS_INTERFACE(obj)         (G_TYPE_CHECK_INSTANCE_TYPE((obj),\
                                              GPOP_TYPE_DBUS_INTERFACE))
#define GPOP_IS_DBUS_INTERFACE_CLASS(klass) (G_TYPE_CHECK_CLASS_TYPE((klass),\
                                              GPOP_TYPE_DBUS_INTERFACE))

typedef struct _GPOPDBusInterface GPOPDBusInterface;
typedef struct _GPOPDBusInterfaceClass GPOPDBusInterfaceClass;

struct _GPOPDBusInterface {
  GObject base;

  guint object_id;
  GDBusConnection* connection;
  GDBusNodeInfo* introspection_data;
  gchar* object_path;
  GDBusInterfaceVTable* interface_vtable;
};

struct _GPOPDBusInterfaceClass
{
  GObjectClass base;
  GDBusInterfaceMethodCallFunc method_call;
  GDBusInterfaceGetPropertyFunc get_property;
  GDBusInterfaceSetPropertyFunc set_property;
};

GType gpop_dbus_interface_get_type (void);

gboolean gpop_dbus_interface_register (GPOPDBusInterface * iface, const gchar* object_path, const gchar* xml_introspection, GDBusConnection * connection);

#endif /* _GPOP_DBUS_INTERFACE_H_ */
