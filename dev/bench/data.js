window.BENCHMARK_DATA = {
  "lastUpdate": 1686801222696,
  "repoUrl": "https://github.com/maidsafe/safe_network",
  "entries": {
    "Safe Network Benchmarks": [
      {
        "commit": {
          "author": {
            "email": "joshuef@gmail.com",
            "name": "Josh Wilson",
            "username": "joshuef"
          },
          "committer": {
            "email": "joshuef@gmail.com",
            "name": "joshuef",
            "username": "joshuef"
          },
          "distinct": true,
          "id": "fe09de5bfbc2fb3639d3285eea4b37ad50393c39",
          "message": "ci: add initial benchmarks for prs and chart generation",
          "timestamp": "2023-06-15T12:16:35+09:00",
          "tree_id": "480a19ea2ffe1d4ff4da682b510550dc180962e8",
          "url": "https://github.com/maidsafe/safe_network/commit/fe09de5bfbc2fb3639d3285eea4b37ad50393c39"
        },
        "date": 1686800670885,
        "tool": "cargo",
        "benches": [
          {
            "name": "Upload Benchmark 1MB/safe files upload/1",
            "value": 1416191888,
            "range": "± 371830459",
            "unit": "ns/iter"
          },
          {
            "name": "Upload Benchmark 10MB/safe files upload/10",
            "value": 3421593781,
            "range": "± 480652340",
            "unit": "ns/iter"
          },
          {
            "name": "Download Benchmark/safe files download",
            "value": 51116998895,
            "range": "± 1034937275",
            "unit": "ns/iter"
          }
        ]
      },
      {
        "commit": {
          "author": {
            "email": "action@github.com",
            "name": "GitHub Action",
            "username": "actions-user"
          },
          "committer": {
            "email": "action@github.com",
            "name": "GitHub Action",
            "username": "actions-user"
          },
          "distinct": true,
          "id": "11ffb58e4ce83c5c6660a7334326e01f9efddf80",
          "message": "chore(release): sn_cli-v0.77.18/sn_node-v0.83.16/sn_testnet-v0.1.20",
          "timestamp": "2023-06-15T03:18:57Z",
          "tree_id": "7f48d02cee5431b7f2869e33d6741c6cd722c104",
          "url": "https://github.com/maidsafe/safe_network/commit/11ffb58e4ce83c5c6660a7334326e01f9efddf80"
        },
        "date": 1686801221566,
        "tool": "cargo",
        "benches": [
          {
            "name": "Upload Benchmark 1MB/safe files upload/1",
            "value": 1973325870,
            "range": "± 292043093",
            "unit": "ns/iter"
          },
          {
            "name": "Upload Benchmark 10MB/safe files upload/10",
            "value": 3684522138,
            "range": "± 467166747",
            "unit": "ns/iter"
          },
          {
            "name": "Download Benchmark/safe files download",
            "value": 58367915702,
            "range": "± 1252551892",
            "unit": "ns/iter"
          }
        ]
      }
    ]
  }
}