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

#ifndef _GSTPOP_PARSER_H_
#define _GSTPOP_PARSER_H_

#define GSTPOP_TYPE_PARSER	           (gstpop_parser_get_type())
#define GSTPOP_PARSER(obj)            (G_TYPE_CHECK_INSTANCE_CAST((obj),\
                                              GSTPOP_TYPE_PARSER, GSTPOPParser))
#define GSTPOP_PARSER_CLASS(klass)    (G_TYPE_CHECK_CLASS_CAST((klass),\
                                              GSTPOP_TYPE_PARSER, GSTPOPParserClass))
#define GSTPOP_PARSER_GET_CLASS(obj)  (G_TYPE_INSTANCE_GET_CLASS ((obj),\
                                              GSTPOP_TYPE_PARSER, GSTPOPParserClass))
#define GSTPOP_IS_PARSER(obj)         (G_TYPE_CHECK_INSTANCE_TYPE((obj),\
                                              GSTPOP_TYPE_PARSER))
#define GSTPOP_IS_PARSER_CLASS(klass) (G_TYPE_CHECK_CLASS_TYPE((klass),\
                                              GSTPOP_TYPE_PARSER))

typedef struct _GSTPOPParser GSTPOPParser;
typedef struct _GSTPOPParserClass GSTPOPParserClass;

typedef enum {
  GSTPOP_PARSER_READY,
  GSTPOP_PARSER_PLAYING,
  GSTPOP_PARSER_PAUSED,
  GSTPOP_PARSER_EOS,
  GSTPOP_PARSER_ERROR,
  GSTPOP_PARSER_LAST,
} GSTPOPParserState;

struct _GSTPOPParserClass
{
  GObjectClass base;

  void (*state_changed) (GSTPOPParser * parser, GSTPOPParserState state);
};

GSTPOPParser * gstpop_parser_new ();
void gstpop_parser_free (GSTPOPParser* parser);
void gstpop_parser_quit (GSTPOPParser * parser);

gboolean gstpop_parser_play (GSTPOPParser *parser, const gchar * parser_desc);

gboolean gstpop_parser_is_playing (GSTPOPParser *parser);

gboolean gstpop_parser_change_state (GSTPOPParser * parser, GSTPOPParserState state);

#endif /* _GSTPOP_PARSER_H_ */
