project('gpop', 'c', version : '0.2.0', license : 'GPL')

pkg = import('pkgconfig')

cc = meson.get_compiler('c')

# Dependencies
glib_req_version = '>= 2.44.0'
gst_req_version = '>= 1.16.0'

glib_dep = dependency('glib-2.0', version: glib_req_version,
    fallback: ['glib', 'libglib_dep'])
gio_dep = [dependency('gio-2.0', version: glib_req_version,
                  fallback: ['glib', 'libgio_dep'])]
if host_machine.system() != 'windows'
  gio_dep += dependency('gio-unix-2.0', version: glib_req_version,
                  fallback: ['glib', 'libgio_dep'])
endif
gobject_dep = dependency('gobject-2.0', version: glib_req_version,
    fallback: ['glib', 'libgobject_dep'])

gst_dep = dependency('gstreamer-1.0', version: gst_req_version,
    fallback : ['gstreamer', 'gst_dep'])

root_inc = include_directories('.')

# Install git hooks for development
meson.add_postconf_script('scripts/setup-hooks.sh')

subdir('lib')

if get_option('cclient')
  subdir('client/c')
endif

# Rust targets (daemon + client)
cargo = find_program('cargo', required: true)

custom_target('gpop',
  output: 'gpop',
  command: [cargo, 'build', '--release', '-p', 'gpop', '--target-dir', '@OUTDIR@'],
  console: true,
  build_by_default: true,
)

if get_option('client')
  custom_target('gpop-client',
    output: 'gpop-client',
    command: [cargo, 'build', '--release', '-p', 'gpop-client', '--target-dir', '@OUTDIR@'],
    console: true,
    build_by_default: true,
  )
endif
