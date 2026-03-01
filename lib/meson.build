
src = ['src/gstpop-main.c'
	   , 'src/gstpop-dbus-interface.c'
	   , 'src/gstpop-manager.c'
	   , 'src/gstpop-pipeline.c'
	   , 'src/gstpop-parser.c'
	   , 'src/gstpop-client.c'
	   ]

inc = [ 'src/gstpop-main.h', 'src/gstpop-client.h']

soup_dep = dependency('libsoup-3.0', version: '>= 3.0', required: true)
json_dep = dependency('json-glib-1.0', version: '>= 1.0', required: true)

libgstpop_dependencies = [
  glib_dep,
  gobject_dep,
  gio_dep,
  gst_dep,
  soup_dep,
  json_dep,
]

libgstpop = library('libgstpop'
				  , src, dependencies : libgstpop_dependencies
				  , install : true)

libgstpop_dep = declare_dependency(
  dependencies: libgstpop_dependencies,
  sources: inc,
  include_directories: include_directories('src'),
  link_with: libgstpop,
)

pkg.generate(name: 'gstpop',
             description: 'Gstreamer Prince Of Parser Library',
             version: meson.project_version(),
             libraries: libgstpop)