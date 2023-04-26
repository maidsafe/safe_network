resource "digitalocean_droplet" "node1" {
  image    = "ubuntu-22-04-x64"
  name     = "${terraform.workspace}-safe-node1"
  region   = var.region
  size     = var.node-size
  ssh_keys = var.ssh_keys

  connection {
    host        = self.ipv4_address
    user        = "root"
    type        = "ssh"
    timeout     = "5m"
    private_key = file(var.pvt_key)
  }

  provisioner "file" {
    source       = "init-node.sh"
    destination  = "/tmp/init-node.sh"
  }

  provisioner "file" {
    source       = "workspace/${terraform.workspace}/node-1"
    destination  = "/node-1-peer-id"
  }


  provisioner "local-exec" {
    command = <<EOH
      mkdir -p ~/.ssh/
      touch ~/.ssh/known_hosts
      echo "node-1 ${self.ipv4_address}" >> workspace/${terraform.workspace}/ip-list
      ssh-keyscan -H ${self.ipv4_address} >> ~/.ssh/known_hosts
    EOH
  }

  # user_data = data.terraform_remote_state.node.outputs[0]
  
    # For a non-genesis node, we pass an empty value for the node IP address.
  # It looks a bit awkward because you have to escape the double quotes.
  provisioner "remote-exec" {
    inline = [
      "sudo DEBIAN_FRONTEND=noninteractive apt install ripgrep -y > /dev/null 2>&1",
      "chmod +x /tmp/init-node.sh",
      "/tmp/init-node.sh \"${var.node_url}\" \"${var.port}\" \"${terraform.workspace}-safe-node-1\"",
      "rg \"listening on \".+\"\" > /tmp/output.txt",
    ]
  }

          # rg for non local ip, and then grab teh whole line, but remove the last character
  provisioner "local-exec" {
         command = "rsync -z root@${self.ipv4_address}:/tmp/output.txt ./workspace/${terraform.workspace}/node-1"
       
    }
  provisioner "local-exec" {
         command = "rg --pcre2 -i '\\b((?!10\\.|172\\.(1[6-9]|2\\d|3[01])\\.|192\\.168\\.|169\\.254\\.|127\\.0\\.0\\.1)[0-9]+\\.[0-9]+\\.[0-9]+\\.[0-9]+).+' workspace/${terraform.workspace}/node-1 -o | sed 's/.$//' > ./workspace/${terraform.workspace}/node-1"
    }
}

resource "digitalocean_droplet" "node" {
  count    = var.number_of_nodes - 1
  image    = "ubuntu-22-04-x64"
  name     = "${terraform.workspace}-safe-node-${count.index + 2}" // 2 because 0 index + initial node1
  region   = var.region
  size     = var.node-size
  ssh_keys = var.ssh_keys
  depends_on = [digitalocean_droplet.node1]
  
  connection {
    host        = self.ipv4_address
    user        = "root"
    type        = "ssh"
    timeout     = "5m"
    private_key = file(var.pvt_key)
  }

  provisioner "file" {
    source       = "init-node.sh"
    destination  = "/tmp/init-node.sh"
  }

  provisioner "file" {
    source       = "workspace/${terraform.workspace}/node-1"
    destination  = "/node-1-peer-id"
  }


  provisioner "local-exec" {
    command = <<EOH
      mkdir -p ~/.ssh/
      touch ~/.ssh/known_hosts
      echo "node-${count.index + 1} ${self.ipv4_address}" >> workspace/${terraform.workspace}/ip-list
      ssh-keyscan -H ${self.ipv4_address} >> ~/.ssh/known_hosts
    EOH
  }

  # user_data = data.terraform_remote_state.node.outputs[0]
  
    # For a non-genesis node, we pass an empty value for the node IP address.
  # It looks a bit awkward because you have to escape the double quotes.
  provisioner "remote-exec" {
    inline = [
      "sudo DEBIAN_FRONTEND=noninteractive apt install ripgrep -y > /dev/null 2>&1",
      "chmod +x /tmp/init-node.sh",
      "/tmp/init-node.sh \"${var.node_url}\" \"${var.port}\" \"${terraform.workspace}-safe-node-${count.index + 1}\" \"/ip4/$(cat /node-1-peer-id)\"",
      "rg \"listening on \".+\"\" > /tmp/output.txt",
    ]
  }
}


# # Use null_resource and remote-exec to execute the command on the first droplet
# # and store the output in a file
# resource "null_resource" "get_first_peer_id" {
#   depends_on = [digitalocean_droplet.node]

#   provisioner "remote-exec" {
#     inline = [
#       "sudo DEBIAN_FRONTEND=noninteractive apt install ripgrep -y > /dev/null 2>&1",
#       "rg \"listening on \".+\"\" > /tmp/output.txt"
#     ]
  
#     connection {
#       type        = "ssh"
#       user        = "root"
#       private_key = file(var.pvt_key)
#       host        = digitalocean_droplet.node.0.ipv4_address
#     }
#   }

#    provisioner "local-exec" {
#         command = "rsync -z root@${digitalocean_droplet.node.0.ipv4_address}:/tmp/output.txt ./workspace/${terraform.workspace}/node-1"
#     }
# }
