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

#ifndef _GSTPOP_PIPELINE_H_
#define _GSTPOP_PIPELINE_H_

#define GSTPOP_TYPE_PIPELINE	           (gstpop_pipeline_get_type())
#define GSTPOP_PIPELINE(obj)            (G_TYPE_CHECK_INSTANCE_CAST((obj),\
                                              GSTPOP_TYPE_PIPELINE, GSTPOPPipeline))
#define GSTPOP_PIPELINE_CLASS(klass)    (G_TYPE_CHECK_CLASS_CAST((klass),\
                                              GSTPOP_TYPE_PIPELINE, GSTPOPPipelineClass))
#define GSTPOP_PIPELINE_GET_CLASS(obj)  (G_TYPE_INSTANCE_GET_CLASS ((obj),\
                                              GSTPOP_TYPE_PIPELINE, GSTPOPPipelineClass))
#define GSTPOP_IS_PIPELINE(obj)         (G_TYPE_CHECK_INSTANCE_TYPE((obj),\
                                              GSTPOP_TYPE_PIPELINE))
#define GSTPOP_IS_PIPELINE_CLASS(klass) (G_TYPE_CHECK_CLASS_TYPE((klass),\
                                              GSTPOP_TYPE_PIPELINE))

typedef struct _GSTPOPPipeline GSTPOPPipeline;
typedef struct _GSTPOPPipelineClass GSTPOPPipelineClass;

struct _GSTPOPPipeline
{
  GSTPOPDBusInterface base;
  GSTPOPParser * parser;
  GSTPOPManager *manager;
  guint num;
  gchar * id;
  gchar * parser_desc;
};

struct _GSTPOPPipelineClass
{
  GSTPOPDBusInterfaceClass base;
};

GSTPOPPipeline * gstpop_pipeline_new (GSTPOPManager* manager, GDBusConnection* connection, guint num);
void gstpop_pipeline_free (GSTPOPPipeline* pipeline);
gboolean gstpop_pipeline_set_state (GSTPOPPipeline* pipeline, GSTPOPParserState state);
gboolean gstpop_pipeline_set_parser_desc (GSTPOPPipeline* pipeline, const gchar * parser_desc);

#endif /* _GSTPOP_PIPELINE_H_ */
