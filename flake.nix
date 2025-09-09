{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flakeCompat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
    nci = {
      url = "github:90-008/nix-cargo-integration";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.parts.follows = "parts";
      inputs.dream2nix.follows = "dream2nix";
      inputs.crane.follows = "crane";
    };
    parts = {
      url = "github:hercules-ci/flake-parts";
      inputs.nixpkgs-lib.follows = "nixpkgs";
    };
    dream2nix = {
      url = "github:NeuralModder/dream2nix/update-crane";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane = {
      url = "github:ipetkov/crane/v0.21.0";
      flake = false;
    };
  };

  outputs = inputs @ {
    parts,
    nci,
    nixpkgs,
    ...
  }: let
    filteredSource = builtins.path {
      name = "airshipper-source";
      path = toString ./.;
      filter = path: type:
        nixpkgs.lib.all
        (n: builtins.baseNameOf path != n)
        [
          ".github"
          ".gitlab"
          ".gitlab-ci.yml"
          "shell.nix"
          "default.nix"
          "flake.lock"
          "flake.nix"
          "TROUBLESHOOTING.md"
          "CONTRIBUTING.md"
          "CHANGELOG.md"
          "CODE_OF_CONDUCT.md"
          "WORKFLOW.md"
          "PACKAGING.md"
          "README.md"
        ];
    };

    makeVoxygenPatcher = pkgs: let
      runtimeLibs = with pkgs; (
        [libxkbcommon udev alsa-lib stdenv.cc.cc.lib libGL vulkan-loader]
        ++ (with xorg; [libxcb libX11 libXrandr libXi libXcursor])
      );
    in
      pkgs.writeShellScript "voxygen-patch" ''
        echo "making veloren-voxygen executable"
        chmod +x veloren-voxygen
        echo "patching veloren-voxygen dynamic linker"
        ${pkgs.patchelf}/bin/patchelf \
          --set-interpreter "${pkgs.stdenv.cc.bintools.dynamicLinker}" \
          --set-rpath "${nixpkgs.lib.makeLibraryPath runtimeLibs}" \
          veloren-voxygen
      '';

    makeServerPatcher = pkgs:
      pkgs.writeShellScript "server-cli-patch" ''
        echo "making veloren-server-cli executable"
        chmod +x veloren-server-cli
        echo "patching veloren-server-cli dynamic linker"
        ${pkgs.patchelf}/bin/patchelf \
          --set-interpreter "${pkgs.stdenv.cc.bintools.dynamicLinker}" \
          veloren-server-cli
      '';
  in
    parts.lib.mkFlake {inherit inputs;} {
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "aarch64-darwin"
        "x86_64-darwin"
      ];
      imports = [nci.flakeModule];
      perSystem = {
        config,
        pkgs,
        ...
      }: let
        outputs = config.nci.outputs;
        voxygenPatcher = makeVoxygenPatcher pkgs;
        serverPatcher = makeServerPatcher pkgs;
        commonMkDerivation = {
          buildInputs = with pkgs; [openssl];
          nativeBuildInputs = with pkgs; [perl pkg-config];
        };
        serverMkDerivation = {
          buildInputs = with pkgs; [sqlite] ++ commonMkDerivation.buildInputs;
          nativeBuildInputs = commonMkDerivation.nativeBuildInputs;
        };
        wrapPatchers = old:
          pkgs.runCommand
          old.name
          {
            meta = old.meta or {};
            passthru =
              (old.passthru or {})
              // {
                unwrapped = old;
              };
            nativeBuildInputs = [pkgs.makeWrapper];
          }
          ''
            cp -rs --no-preserve=mode,ownership ${old} $out
            wrapProgram $out/bin/* \
              --set VELOREN_VOXYGEN_PATCHER ${voxygenPatcher} \
              --set VELOREN_SERVER_CLI_PATCHER ${serverPatcher} \
          '';
        airshipper = wrapPatchers outputs."airshipper".packages.release;
      in {
        devShells.default = outputs."airshipper".devShell;
        packages.default = airshipper;
        packages.airshipper = airshipper;
        packages.airshipper-dev = wrapPatchers outputs."airshipper".packages.dev;
        packages.airshipper-server-dev = outputs."airshipper-server".packages.dev;
        packages.airshipper-server-release = outputs."airshipper-server".packages.release;

        nci.projects."airshipper" = {
          export = true;
          path = filteredSource;
        };

        nci.crates."airshipper" = {
          export = false;
          runtimeLibs = with pkgs;
            [
              libxkbcommon
              vulkan-loader
              wayland
              wayland-protocols
              xorg.libX11
              xorg.libXrandr
              xorg.libXi
              xorg.libXcursor
            ]
            ++ commonMkDerivation.buildInputs;
          depsDrvConfig.mkDerivation = commonMkDerivation;
          drvConfig.mkDerivation = commonMkDerivation;
        };

        nci.crates."airshipper-server" = {
          # Need to reexport since defining runtimeLibs here causes a strange error with tests or clippy
          export = false;
          runtimeLibs = serverMkDerivation.buildInputs;
          depsDrvConfig.mkDerivation = serverMkDerivation;
          drvConfig.mkDerivation = serverMkDerivation;
        };
      };
    };
}
