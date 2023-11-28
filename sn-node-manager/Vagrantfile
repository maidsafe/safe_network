Vagrant.configure("2") do |config|
  config.vm.box = "generic/ubuntu2204"
  config.vm.provider :libvirt do |libvirt|
    libvirt.memory = 4096
  end
  config.vm.synced_folder ".",
    "/vagrant",
    type: "9p",
    accessmode: "mapped",
    mount_options: ['rw', 'trans=virtio', 'version=9p2000.L']
  config.vm.provision "file", source: "~/.ssh/id_rsa", destination: "/home/vagrant/.ssh/id_rsa"
  config.vm.provision "shell", inline: "apt-get update -y"
  config.vm.provision "shell", inline: "apt-get install -y build-essential"
  config.vm.provision "shell", privileged: false, inline: <<-SHELL
    curl -L -O https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
    chmod +x rustup-init
    ./rustup-init --default-toolchain stable --no-modify-path -y
    echo "source ~/.cargo/env" >> ~/.bashrc
  SHELL
  config.vm.provision "shell", inline: <<-SHELL
    curl -L -O https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-gnu/rustup-init
    chmod +x rustup-init
    ./rustup-init --default-toolchain stable --no-modify-path -y
    echo "source ~/.cargo/env" >> ~/.bashrc
    # Copy the binaries to a system-wide location for running tests as the root user
    sudo cp ~/.cargo/bin/** /usr/local/bin
  SHELL
  config.vm.provision "shell", privileged: false, inline: <<-SHELL
    mkdir -p ~/.vim/tmp/ ~/.vim/backup
    cat <<'EOF' > ~/.vimrc
set nocompatible

let mapleader=" "
syntax on

set background=dark
set backspace=indent,eol,start
set backupdir=~/.vim/tmp//
set directory=~/.vim/backup
set expandtab
set foldlevel=1
set foldmethod=indent
set foldnestmax=10
set hlsearch
set ignorecase
set incsearch
set laststatus=2
set nobackup
set nofoldenable
set nowrap
set number relativenumber
set ruler
set shiftwidth=4
set smartindent
set showcmd
set shortmess+=A
set tabstop=4
set viminfo+=!

nnoremap j gj
nnoremap k gk
EOF
  SHELL
end
