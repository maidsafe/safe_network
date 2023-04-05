resource "digitalocean_droplet" "testnet_node" {
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

  # For a non-genesis node, we pass an empty value for the node IP address.
  # It looks a bit awkward because you have to escape the double quotes.
  provisioner "remote-exec" {
    inline = [
      "chmod +x /tmp/init-node.sh",
      "/tmp/init-node.sh \"${var.node_url}\" \"${var.port}\" \"${terraform.workspace}-safe-node-${count.index + 1}\" \"${digitalocean_droplet.testnet_node[0].ipv4_address}\"",
    ]
  }

  provisioner "local-exec" {
    command = <<EOH
      mkdir -p ~/.ssh/
      touch ~/.ssh/known_hosts
      echo "node-${count.index + 2} ${self.ipv4_address}" >> workspace/${terraform.workspace}/ip-list
      ssh-keyscan -H ${self.ipv4_address} >> ~/.ssh/known_hosts
    EOH
  }
}
