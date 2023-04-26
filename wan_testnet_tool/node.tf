resource "digitalocean_droplet" "node" {
  count    = var.number_of_nodes
  image    = "ubuntu-22-04-x64"
  name     = "${terraform.workspace}-safe-node-${count.index + 1}" // 1 because 0 index
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

  provisioner "local-exec" {
    # Create an empty file if it does not exist
    command = "cat workspace/${terraform.workspace}/node-1 || echo '' > workspace/${terraform.workspace}/node-1"
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
      "chmod +x /tmp/init-node.sh",
      "/tmp/init-node.sh \"${var.node_url}\" \"${var.port}\" \"${terraform.workspace}-safe-node-${count.index + 1}\" $(cat /node-1-peer-id)",
    ]
  }
}


# Use null_resource and remote-exec to execute the command on the first droplet
# and store the output in a file
resource "null_resource" "get_first_peer_id" {
  depends_on = [digitalocean_droplet.node]

  provisioner "remote-exec" {
    inline = [
      "rg \"listening on \".+\"\" | rg \"\".+\"\" -o > /tmp/output.txt"
    ]
  
    connection {
      type        = "ssh"
      user        = "root"
      private_key = file(var.pvt_key)
      host        = digitalocean_droplet.node.0.ipv4_address
    }
  }

   provisioner "local-exec" {
        command = "rsync -z root@${digitalocean_droplet.node.0.ipv4_address}:/tmp/output.txt ./workspace/node-1"
    }
}

