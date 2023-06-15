window.BENCHMARK_DATA = {
  "lastUpdate": 1686800671371,
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
      }
    ]
  }
}