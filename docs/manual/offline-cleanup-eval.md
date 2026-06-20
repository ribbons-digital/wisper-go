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

| Case | Spoken content | Expected behavior | Raw ASR | Model suggestion | Safety decision | Final inserted output | ASR ms | Cleanup ms | Safety notes | Quality notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| English sentence | today we reviewed the release checklist and fixed the last offline inference issue | Final output must preserve all words; accepted suggestion may add sentence capitalization/punctuation. |  |  |  |  |  |  |  |  |
| English question | can you send the updated notes before the meeting starts | Final output must preserve all words; accepted suggestion may add capitalization and a question mark. |  |  |  |  |  |  |  |  |
| Chinese sentence | 今天我们完成了离线语音识别和标点清理测试 | Final output must preserve every Chinese character in order; accepted suggestion may add appropriate Chinese punctuation. |  |  |  |  |  |  |  |  |
| Chinese question | 你明天可以帮我检查这个离线版本吗 | Final output must preserve every Chinese character in order; accepted suggestion may add appropriate Chinese question punctuation. |  |  |  |  |  |  |  |  |
| Mixed English Chinese | please remind 小王 to review the offline build tonight | Final output must preserve English words and Chinese characters exactly; unsafe suggestions fall back to raw ASR. |  |  |  |  |  |  |  |  |
| Already punctuated | Wispergo already works offline, and cleanup should not rewrite this sentence. | Final output should preserve existing words and punctuation unless a safe punctuation-only change is clearly beneficial. |  |  |  |  |  |  |  |  |

## Pass criteria

- Final inserted output does not translate between languages or scripts.
- Final inserted output does not meaningfully add, remove, or rewrite words.
- Safe raw-ASR fallback counts as a safety pass when model suggestion is unsafe.
- Punctuation quality is recorded separately from safety; lack of punctuation improvement is not a safety failure.
- For Punctuation-only on Apple Silicon, the safety decision and final output should complete within the product Punctuation-only cleanup timeout of 1200ms for warm, lifecycle-managed cleanup. If cleanup exceeds 1200ms, product behavior is raw-ASR fallback and quality should be marked as fallback/no improvement.
- Standalone eval runs that include cold model load should record cold-load latency separately in notes; do not treat cold-load time as normal dictation latency.
- Intel Macs may pass safety by falling back to raw ASR when cleanup exceeds the 1200ms Punctuation-only timeout.
