project('gst-pop', 'c',
  version : run_command('sh', '-c',
    'grep -A1 "\\[workspace.package\\]" Cargo.toml | grep version | sed \'s/.*"\\(.*\\)"/\\1/\'',
    check: true).stdout().strip(),
  license : 'GPL')

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

# Install git hooks for development
meson.add_postconf_script('scripts/setup-hooks.sh')

subdir('lib')

if get_option('cclient')
  subdir('client/c')
endif

# Rust targets (daemon + client)
cargo = find_program('cargo', required: true)

custom_target('gst-pop',
  output: 'gst-pop',
  command: [cargo, 'build', '--release', '-p', 'gst-pop', '--target-dir', '@OUTDIR@'],
  console: true,
  build_by_default: true,
)

# Install the cargo-built binary and create busybox-style symlinks (Unix only)
meson.add_install_script('sh', '-c',
  'install -Dm755 "@0@" "$DESTDIR@1@/gst-pop"'.format(
    meson.project_build_root() / 'release' / 'gst-pop',
    get_option('prefix') / get_option('bindir')))

if host_machine.system() != 'windows'
  foreach link_name : ['gst-popd', 'gst-pop-launch', 'gst-pop-inspect', 'gst-pop-discover', 'gst-pop-play']
    meson.add_install_script('sh', '-c',
      'ln -sf gst-pop "$DESTDIR@0@/@1@"'.format(
        get_option('prefix') / get_option('bindir'), link_name))
  endforeach
endif

# Install systemd user service (Linux only)
if host_machine.system() == 'linux'
  systemd_user_dir = join_paths(get_option('prefix'), 'lib', 'systemd', 'user')
  configure_file(
    input: 'data/gst-pop.service.in',
    output: 'gst-pop.service',
    configuration: {
      'bindir': get_option('prefix') / get_option('bindir'),
    },
    install: true,
    install_dir: systemd_user_dir,
  )
endif

if get_option('client')
  custom_target('gst-popctl',
    output: 'gst-popctl',
    command: [cargo, 'build', '--release', '-p', 'gst-popctl', '--target-dir', '@OUTDIR@'],
    console: true,
    build_by_default: true,
  )

  meson.add_install_script('sh', '-c',
    'install -Dm755 "@0@" "$DESTDIR@1@/gst-popctl"'.format(
      meson.project_build_root() / 'release' / 'gst-popctl',
      get_option('prefix') / get_option('bindir')))
endif
