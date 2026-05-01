# Offline Cleanup Manual Evaluation

Use this fixture to compare raw ASR output against bundled offline cleanup output and record latency on each target Mac.

## Environment checklist

- [ ] App build: `____________________`
- [ ] Mac model: `____________________`
- [ ] Architecture: `Apple Silicon / Intel`
- [ ] ASR model: `ggml-large-v3-turbo / other: ____________________`
- [ ] Cleanup model: `Qwen2.5-3B-Instruct GGUF / other: ____________________`
- [ ] Cleanup mode: `Punctuation only / Full cleanup / Off`

## Evaluation cases

| Case | Spoken content | Expected cleanup behavior | Raw ASR | Cleanup output | ASR ms | Cleanup ms | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- |
| English sentence | today we reviewed the release checklist and fixed the last offline inference issue | Add sentence capitalization and punctuation only. |  |  |  |  |  |
| English question | can you send the updated notes before the meeting starts | Add capitalization and a question mark only. |  |  |  |  |  |
| Chinese sentence | 今天我们完成了离线语音识别和标点清理测试 | Add appropriate Chinese punctuation only. |  |  |  |  |  |
| Chinese question | 你明天可以帮我检查这个离线版本吗 | Add appropriate Chinese question punctuation only. |  |  |  |  |  |
| Mixed English Chinese | please remind 小王 to review the offline build tonight | Preserve English and Chinese text, add punctuation/capitalization only. |  |  |  |  |  |
| Already punctuated | Wispergo already works offline, and cleanup should not rewrite this sentence. | Leave existing words and punctuation intact except for clearly necessary minimal cleanup. |  |  |  |  |  |

## Pass criteria

- Cleanup does not translate between languages or scripts.
- Cleanup does not meaningfully add, remove, or rewrite words.
- Apple Silicon latency is acceptable for normal dictation use.
- Intel Macs may fall back to raw ASR when cleanup is unavailable or too slow.
