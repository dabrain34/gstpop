# C WebSocket client for gpop

# readline doesn't have pkg-config, use compiler.find_library
cc = meson.get_compiler('c')
readline_dep = cc.find_library('readline', required: true)

gpop_client_c_sources = [
  'src/main.c',
]

gpop_client_c_deps = [
  libgpop_dep,
  readline_dep,
]

executable('gpop-client-c',
  gpop_client_c_sources,
  dependencies: gpop_client_c_deps,
  install: true,
)
