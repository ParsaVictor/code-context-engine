"""SweRankEmbed-Small (Salesforce, 137M, NomicBERT) on CPU with plain torch.

The published checkpoint needs `trust_remote_code` (NomicBERT modelling code
fetched from another repository); this re-implements its forward pass instead
(post-norm encoder, rotary attention base 1000 on the full head, SwiGLU MLP, no
biases in attention/MLP, token-type embeddings, CLS pooling, L2 norm), so only
the weights and tokenizer are read from
`%LOCALAPPDATA%/neuromesh/models/swerank-embed-small`. Queries carry the model
card's instruction prefix (`QUERY_PREFIX`); documents carry none.
"""
import json
import os

import numpy as np

MODELS = os.path.join(os.environ.get("LOCALAPPDATA", ""), "neuromesh", "models")
QUERY_PREFIX = "Represent this query for searching relevant code: "


class SweRankEmbedder:
    def __init__(self, model="swerank-embed-small", max_tokens=512, threads=0):
        import torch
        from safetensors.torch import load_file
        from tokenizers import Tokenizer

        self.torch = torch
        if threads:
            torch.set_num_threads(threads)
        d = os.path.join(MODELS, model)
        cfg = json.load(open(os.path.join(d, "config.json"), encoding="utf-8"))
        self.n_layer, self.n_head, self.dim = cfg["n_layer"], cfg["n_head"], cfg["n_embd"]
        self.eps = cfg["layer_norm_epsilon"]
        self.base = float(cfg["rotary_emb_base"])
        sd = load_file(os.path.join(d, "model.safetensors"))
        self.w = {k.removeprefix("bert."): v.float() for k, v in sd.items()}
        self.tok = Tokenizer.from_file(os.path.join(d, "tokenizer.json"))
        self.tok.no_padding()
        self.tok.enable_truncation(max_tokens)
        self.max_tokens = max_tokens
        hd = self.dim // self.n_head
        self.inv_freq = 1.0 / (self.base ** (torch.arange(0, hd, 2, dtype=torch.float32) / hd))

    def _ln(self, x, name):
        F = self.torch.nn.functional
        return F.layer_norm(x, (self.dim,), self.w[name + ".weight"], self.w[name + ".bias"], self.eps)

    def _lin(self, x, name):
        b = self.w.get(name + ".bias")
        y = x @ self.w[name + ".weight"].T
        return y + b if b is not None else y

    def _rotary(self, x, cos, sin):
        x1, x2 = x.chunk(2, dim=-1)
        return x * cos + self.torch.cat((-x2, x1), dim=-1) * sin

    def forward(self, ids, mask):
        torch = self.torch
        F = torch.nn.functional
        w = self.w
        b, s = ids.shape
        h = w["embeddings.word_embeddings.weight"][ids] + w["embeddings.token_type_embeddings.weight"][0]
        h = self._ln(h, "emb_ln")
        freqs = torch.outer(torch.arange(s, dtype=torch.float32), self.inv_freq)
        cos = torch.cat((freqs.cos(), freqs.cos()), -1)[None, None]
        sin = torch.cat((freqs.sin(), freqs.sin()), -1)[None, None]
        bias = (1.0 - mask[:, None, None, :].float()) * torch.finfo(torch.float32).min
        hd = self.dim // self.n_head
        for i in range(self.n_layer):
            p = f"encoder.layers.{i}."
            qkv = self._lin(h, p + "attn.Wqkv").view(b, s, 3, self.n_head, hd).permute(2, 0, 3, 1, 4)
            q, k, v = self._rotary(qkv[0], cos, sin), self._rotary(qkv[1], cos, sin), qkv[2]
            a = F.scaled_dot_product_attention(q, k, v, attn_mask=bias)
            a = self._lin(a.transpose(1, 2).reshape(b, s, self.dim), p + "attn.out_proj")
            h = self._ln(h + a, p + "norm1")
            m = self._lin(h, p + "mlp.fc11") * F.silu(self._lin(h, p + "mlp.fc12"))
            h = self._ln(h + self._lin(m, p + "mlp.fc2"), p + "norm2")
        return h[:, 0]

    def embed(self, texts, max_tokens=None, batch=16):
        torch = self.torch
        enc = [self.tok.encode(t).ids for t in texts]
        order = sorted(range(len(texts)), key=lambda i: len(enc[i]))
        out = np.zeros((len(texts), self.dim), dtype=np.float32)
        with torch.inference_mode():
            for st in range(0, len(order), batch):
                idx = order[st:st + batch]
                n = max(len(enc[i]) for i in idx)
                arr = torch.zeros((len(idx), n), dtype=torch.long)
                mask = torch.zeros((len(idx), n), dtype=torch.long)
                for j, i in enumerate(idx):
                    arr[j, :len(enc[i])] = torch.tensor(enc[i])
                    mask[j, :len(enc[i])] = 1
                v = torch.nn.functional.normalize(self.forward(arr, mask), dim=-1)
                out[idx] = v.numpy()
        return out
