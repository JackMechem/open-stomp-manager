{
  description = "Open Stomp Manager: open editor for the SORAN Dual Stomp (Rust + egui)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    # the pedal library, a path dependency on a sibling checkout (../open-stomp-lib).
    # Local checkout instead: nix run --override-input open-stomp-lib path:../open-stomp-lib
    open-stomp-lib = {
      url = "github:JackMechem/open-stomp-lib";
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
        inherit (pkgs.stdenv.hostPlatform) isLinux;

        # Libraries dlopened at runtime on Linux: X11 or Wayland and OpenGL (winit/glow), and
        # libdbus for rfd's file dialog (xdg-desktop-portal).
        runtimeLibs = lib.optionals isLinux (
          with pkgs;
          [
            libGL
            libxkbcommon
            wayland
            libx11
            libxcursor
            libxi
            libxrandr
            vulkan-loader
            dbus
          ]
        );

        buildInputs = lib.optionals isLinux [ pkgs.alsa-lib ] ++ runtimeLibs;
        nativeBuildInputs = [ pkgs.pkg-config ] ++ lib.optionals isLinux [ pkgs.makeWrapper ];

        appSrc = lib.cleanSourceWith {
          src = ./.;
          filter =
            path: type:
            let
              rel = lib.removePrefix (toString ./. + "/") (toString path);
            in
            rel == "Cargo.toml" || rel == "Cargo.lock" || rel == "crates" || lib.hasPrefix "crates/" rel;
        };

        open-stomp-manager = pkgs.rustPlatform.buildRustPackage {
          pname = "open-stomp-manager";
          version = (lib.importTOML ./Cargo.toml).workspace.package.version;
          # this repo and the library side by side, as cargo expects them
          src = pkgs.runCommand "open-stomp-src" { } ''
            mkdir -p $out
            cp -r ${appSrc} $out/open-stomp-manager
            cp -r ${open-stomp-lib} $out/open-stomp-lib
          '';
          sourceRoot = "open-stomp-src/open-stomp-manager";
          cargoLock.lockFile = ./Cargo.lock;
          cargoBuildFlags = [ "-p" "open-stomp-manager" ];
          inherit buildInputs nativeBuildInputs;

          postInstall = lib.optionalString isLinux ''
            wrapProgram $out/bin/open-stomp-manager \
              --prefix LD_LIBRARY_PATH : ${lib.makeLibraryPath runtimeLibs}
            install -Dm644 ${./packaging/open-stomp-manager.desktop} $out/share/applications/open-stomp-manager.desktop
            install -Dm644 ${./packaging/icons/icon.png} $out/share/pixmaps/open-stomp-manager.png
          '';

          meta = {
            description = "Open editor / live monitor for the SORAN Dual Stomp guitar pedal";
            license = with lib.licenses; [ mit asl20 ];
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
          inherit buildInputs;
          nativeBuildInputs =
            nativeBuildInputs
            ++ (with pkgs; [
              cargo
              rustc
              rustfmt
              clippy
              rust-analyzer
            ])
            ++ lib.optionals isLinux [ pkgs.alsa-utils ]; # amidi / aseqdump for poking the pedal
          RUST_SRC_PATH = "${pkgs.rustPlatform.rustLibSrc}";
          LD_LIBRARY_PATH = lib.makeLibraryPath runtimeLibs;
        };

        formatter = pkgs.nixfmt-rfc-style;
      }
    );
}
