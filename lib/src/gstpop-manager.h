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

#ifndef _GSTPOP_MANAGER_H_
#define _GSTPOP_MANAGER_H_

#define GSTPOP_TYPE_MANAGER	           (gstpop_manager_get_type())
#define GSTPOP_MANAGER(obj)            (G_TYPE_CHECK_INSTANCE_CAST((obj),\
                                              GSTPOP_TYPE_MANAGER, GSTPOPManager))
#define GSTPOP_MANAGER_CLASS(klass)    (G_TYPE_CHECK_CLASS_CAST((klass),\
                                              GSTPOP_TYPE_MANAGER, GSTPOPManagerClass))
#define GSTPOP_MANAGER_GET_CLASS(obj)  (G_TYPE_INSTANCE_GET_CLASS ((obj),\
                                              GSTPOP_TYPE_MANAGER, GSTPOPManagerClass))
#define GSTPOP_IS_MANAGER(obj)         (G_TYPE_CHECK_INSTANCE_TYPE((obj),\
                                              GSTPOP_TYPE_MANAGER))
#define GSTPOP_IS_MANAGER_CLASS(klass) (G_TYPE_CHECK_CLASS_TYPE((klass),\
                                              GSTPOP_TYPE_MANAGER))

typedef struct _GSTPOPManager GSTPOPManager;
typedef struct _GSTPOPManagerClass GSTPOPManagerClass;

struct _GSTPOPManager {
  GSTPOPDBusInterface base;
  GList* pipelines;
};

struct _GSTPOPManagerClass
{
  GSTPOPDBusInterfaceClass base;
};

GSTPOPManager* gstpop_manager_new (GDBusConnection* connection);
void gstpop_manager_free (GSTPOPManager * manager);

void gstpop_manager_add_pipeline (GSTPOPManager* manager, guint num, const gchar * parser_desc, gchar* id);
void gstpop_manager_remove_pipeline (GSTPOPManager * manager, gchar* id);
#endif /* _GSTPOP_MANAGER_H_ */
