# C WebSocket client for gst-pop

# readline doesn't have pkg-config, use compiler.find_library
cc = meson.get_compiler('c')
readline_dep = cc.find_library('readline', required: true)

gstpop_client_c_sources = [
  'src/main.c',
]

gstpop_client_c_deps = [
  libgstpop_dep,
  readline_dep,
]

executable('gstpop-client-c',
  gstpop_client_c_sources,
  dependencies: gstpop_client_c_deps,
  install: true,
)
