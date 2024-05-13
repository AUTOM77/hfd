# hfd

[![GitHub Workflow Status (with event)](https://img.shields.io/github/actions/workflow/status/AUTOM77/hfd/ci.yml)](https://github.com/AUTOM77/hfd/actions)
[![GitHub license](https://img.shields.io/github/license/AUTOM77/hfd)](./LICENSE)
[![GitHub contributors](https://img.shields.io/github/contributors/AUTOM77/hfd)](https://github.com/AUTOM77/hfd/graphs/contributors)
[![GitHub commit activity (branch)](https://img.shields.io/github/commit-activity/m/AUTOM77/hfd)](https://github.com/AUTOM77/hfd/commits)
[![GitHub top language](https://img.shields.io/github/languages/top/AUTOM77/hfd?logo=rust&label=)](./hfd-cli/Cargo.toml#L4)
[![Open Issues](https://img.shields.io/github/issues/AUTOM77/hfd)](https://github.com/AUTOM77/hfd/issues)
[![Code Size](https://img.shields.io/github/languages/code-size/AUTOM77/hfd)](.)
[![GitHub all releases](https://img.shields.io/github/downloads/AUTOM77/hfd/total?logo=github)](https://github.com/AUTOM77/hfd/releases)  
[![GitHub release (with filter)](https://img.shields.io/github/v/release/AUTOM77/hfd?logo=github)](https://github.com/AUTOM77/hfd/releases)


🎈Rust-based interface for Huggingface 🤗 download.

> For public, 
`./hdf https://huggingface.co/deepseek-ai/DeepSeek-V2`

> For Gated
`./hdf https://huggingface.co/meta-llama/Meta-Llama-3-70B -t hf_xxxxxxxxxx`

> For Custom Save path
`./hdf https://huggingface.co/meta-llama/Meta-Llama-3-70B -t hf_xxxxxxxxxx -d /data/llm`

> Download with mirror
`./hdf https://huggingface.co/meta-llama/Meta-Llama-3-70B -t hf_xxxxxxxxxx -d /data/llm -m hf-mirror.com`

For a more convinent user experience, execute:

```bash
cat <<EOF | sudo tee -a /etc/security/limits.conf
root soft nofile 20000000
root hard nofile 20000000
*       hard    nofile  20000000
*       soft    nofile  20000000
EOF

cat <<EOF | sudo tee /etc/sysctl.d/bbr.conf
net.core.default_qdisc=fq_codel
net.ipv4.tcp_congestion_control=bbr
net.ipv4.tcp_moderate_rcvbuf = 1
net.ipv4.tcp_mem = '10000000 10000000 10000000'
net.ipv4.tcp_rmem = '1024 4096 16384'
net.ipv4.tcp_wmem = '1024 4096 16384'

net.core.wmem_max = 26214400
net.core.rmem_max = 26214400

fs.file-max = 12000500
fs.nr_open = 20000500
EOF
```

- https://gist.github.com/mustafaturan/47268d8ad6d56cadda357e4c438f51ca