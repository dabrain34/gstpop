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

#ifndef _GSTPOP_DBUS_INTERFACE_H_
#define _GSTPOP_DBUS_INTERFACE_H_

#include <gio/gio.h>
#include <glib.h>

#define GSTPOP_TYPE_DBUS_INTERFACE    (gstpop_dbus_interface_get_type())
#define GSTPOP_DBUS_INTERFACE(obj)            (G_TYPE_CHECK_INSTANCE_CAST((obj),\
                                              GSTPOP_TYPE_DBUS_INTERFACE, GSTPOPDBusInterface))
#define GSTPOP_DBUS_INTERFACE_CLASS(klass)    (G_TYPE_CHECK_CLASS_CAST((klass),\
                                              GSTPOP_TYPE_DBUS_INTERFACE, GSTPOPDBusInterfaceClass))
#define GSTPOP_DBUS_INTERFACE_GET_CLASS(obj)  (G_TYPE_INSTANCE_GET_CLASS ((obj),\
                                              GSTPOP_TYPE_DBUS_INTERFACE, GSTPOPDBusInterfaceClass))
#define GSTPOP_IS_DBUS_INTERFACE(obj)         (G_TYPE_CHECK_INSTANCE_TYPE((obj),\
                                              GSTPOP_TYPE_DBUS_INTERFACE))
#define GSTPOP_IS_DBUS_INTERFACE_CLASS(klass) (G_TYPE_CHECK_CLASS_TYPE((klass),\
                                              GSTPOP_TYPE_DBUS_INTERFACE))

typedef struct _GSTPOPDBusInterface GSTPOPDBusInterface;
typedef struct _GSTPOPDBusInterfaceClass GSTPOPDBusInterfaceClass;

struct _GSTPOPDBusInterface {
  GObject base;

  guint object_id;
  GDBusConnection* connection;
  GDBusNodeInfo* introspection_data;
  gchar* object_path;
  GDBusInterfaceVTable* interface_vtable;
};

struct _GSTPOPDBusInterfaceClass
{
  GObjectClass base;
  GDBusInterfaceMethodCallFunc method_call;
  GDBusInterfaceGetPropertyFunc get_property;
  GDBusInterfaceSetPropertyFunc set_property;
};

GType gstpop_dbus_interface_get_type (void);

gboolean gstpop_dbus_interface_register (GSTPOPDBusInterface * iface, const gchar* object_path, const gchar* xml_introspection, GDBusConnection * connection);

#endif /* _GSTPOP_DBUS_INTERFACE_H_ */
