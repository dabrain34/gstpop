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

#ifndef _GSTPOP_PRIVATE_H_
#define _GSTPOP_PRIVATE_H_

#include <gio/gio.h>
#include <glib.h>

#include "gstpop-dbus-interface.h"
#include "gstpop-manager.h"
#include "gstpop-parser.h"
#include "gstpop-pipeline.h"
#include <gst/gst.h>


#define GSTPOP_LOG(FMT, ...) do { \
      g_print(FMT "\n", ##__VA_ARGS__); \
    } while (0)


#endif /* _GSTPOP_PRIVATE_H_ */
