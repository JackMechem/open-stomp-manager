{
  description = "Open Stomp Manager: open editor for the SORAN Dual Stomp (Rust + Tauri + React)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    # the pedal library, a path dependency on a sibling checkout (../open-stomp-lib). Follows its
    # stable channel; flake.lock pins the release (the one OPEN_STOMP_LIB_REF in the workflows names).
    # Local checkout instead: nix run --override-input open-stomp-lib path:../open-stomp-lib
    open-stomp-lib = {
      url = "github:JackMechem/open-stomp-lib/stable";
      flake = false;
    };
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      open-stomp-lib,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        lib = pkgs.lib;
        inherit (pkgs.stdenv.hostPlatform) isLinux isDarwin;
        version = (lib.importTOML ./Cargo.toml).workspace.package.version;

        # ALSA for MIDI, D-Bus for Bluetooth (BlueZ), and the system webview (WebKitGTK) with what
        # GTK's file chooser needs (GSettings schemas).
        linuxLibs = with pkgs; [
          alsa-lib
          dbus
          webkitgtk_4_1
          gtk3
          libsoup_3
          glib
          glib-networking
          librsvg
          openssl
          gsettings-desktop-schemas
        ];

        # Only what the build reads: the Rust workspace and the app (no node_modules, dist or
        # generated Tauri schemas).
        appSrc = lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            let
              rel = lib.removePrefix (toString ./. + "/") (toString path);
              skipped = [
                "app/node_modules"
                "app/dist"
                "app/src-tauri/gen"
              ];
            in
            (
              rel == "Cargo.toml"
              || rel == "Cargo.lock"
              || rel == "app"
              || lib.hasPrefix "app/" rel
            )
            && !lib.any (s: rel == s || lib.hasPrefix "${s}/" rel) skipped;
        };

        # The frontend's npm packages, pinned by app/pnpm-lock.yaml. After changing the lock file,
        # set `hash` to "" and build once; Nix prints the new hash.
        pnpmDeps = pkgs.fetchPnpmDeps {
          pname = "open-stomp-manager-frontend";
          inherit version;
          src = lib.fileset.toSource {
            root = ./app;
            fileset = lib.fileset.unions [
              ./app/package.json
              ./app/pnpm-lock.yaml
            ];
          };
          fetcherVersion = 4;
          hash = "sha256-yoACHNwtfXIxufnmZxiozzXVDTX5/tYfpADEq9R2Ffo=";
        };

        open-stomp-manager = pkgs.rustPlatform.buildRustPackage {
          pname = "open-stomp-manager";
          inherit version pnpmDeps;
          # this repo and the library side by side, as cargo expects them
          src = pkgs.runCommand "open-stomp-src" { } ''
            mkdir -p $out
            cp -r ${appSrc} $out/open-stomp-manager
            cp -r ${open-stomp-lib} $out/open-stomp-lib
          '';
          sourceRoot = "open-stomp-src/open-stomp-manager";
          cargoLock.lockFile = ./Cargo.lock;

          # `cargo tauri build` (cargo-tauri.hook) builds the frontend (pnpm build) and the app, then
          # installs the .deb's contents (binary, .desktop file, icons) or the macOS .app
          buildAndTestSubdir = "app/src-tauri";
          pnpmRoot = "app";

          nativeBuildInputs = [
            pkgs.cargo-tauri.hook
            pkgs.nodejs
            pkgs.pnpm
            pkgs.pnpmConfigHook
            pkgs.pkg-config
          ]
          ++ lib.optionals isLinux [ pkgs.wrapGAppsHook3 ];
          buildInputs = lib.optionals isLinux linuxLibs;

          postInstall = lib.optionalString isDarwin ''
            mkdir -p $out/bin
            ln -s "$out/Applications/Open Stomp Manager.app/Contents/MacOS/open-stomp-manager" $out/bin/open-stomp-manager
          '';

          meta = {
            description = "Open editor and live monitor for the SORAN Dual Stomp guitar pedal";
            homepage = "https://github.com/JackMechem/open-stomp-manager";
            license = with lib.licenses; [
              mit
              asl20
            ];
            mainProgram = "open-stomp-manager";
            platforms = lib.platforms.linux ++ lib.platforms.darwin;
          };
        };
      in
      {
        packages.default = open-stomp-manager;
        packages.open-stomp-manager = open-stomp-manager;

        apps.default = flake-utils.lib.mkApp { drv = open-stomp-manager; };

        devShells.default = pkgs.mkShell {
          buildInputs = lib.optionals isLinux linuxLibs;
          nativeBuildInputs =
            (with pkgs; [
              pkg-config
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
              nodejs
              pnpm
            ])
            ++ lib.optionals isLinux [
              pkgs.alsa-utils # amidi / aseqdump for poking the pedal
              pkgs.xvfb # off-screen display for tools/dev/shot.py
            ];
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          # GTK's file chooser (Import NAM / IR) aborts without GSettings schemas
          shellHook = lib.optionalString isLinux ''
            export XDG_DATA_DIRS=${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:''${XDG_DATA_DIRS:-/usr/local/share:/usr/share}
          '';
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
