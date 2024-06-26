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

```sh
# Download entire public hf repo
./hdf https://huggingface.co/deepseek-ai/DeepSeek-V2

# Download public hf repo with limit num
./hdf https://huggingface.co/microsoft/Florence-2-large -n 10

# Download gated public hf repo with token
./hdf https://huggingface.co/meta-llama/Meta-Llama-3-70B -t hf_xxxxxxxxxx

# Download gated public hf repo with token and save to /data/llm
./hdf https://huggingface.co/meta-llama/Meta-Llama-3-70B -t hf_xxxxxxxxxx -d /data/llm

# Sometimes, use mirror, for example, hf-mirror.com. 
# The following two options are feasible.
./hdf https://huggingface.co/meta-llama/Meta-Llama-3-70B -t hf_xxxxxxxxxx -d /data/llm -m hf-mirror.com
./hdf https://hf-mirror.com/meta-llama/Meta-Llama-3-70B -t hf_xxxxxxxxxx -d /data/llm
```

For a more convinent user experience, execute:

```sh
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

> [!TIP]
> v0.2.6 Processing time: 4457.868372796s <br/>
> v0.2.7 Processing time: 4441.127406732s <br/>
> v0.2.8 Processing time: 4650.917674426s <br/>

- https://github.com/hyperium/hyper/issues/1358
- https://github.com/hyperium/hyper/issues/1358#issuecomment-366550636
- https://gist.github.com/klausi/f94b9aff7d36a1cb4ebbca746f0a099f
- https://gist.github.com/mustafaturan/47268d8ad6d56cadda357e4c438f51ca

