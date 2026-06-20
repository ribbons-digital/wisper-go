# Offline Cleanup Manual Evaluation

Use this fixture to compare raw ASR output against bundled offline cleanup output and record latency on each target Mac.

## Environment checklist

- [x] App build: `phase-5-2-cleanup-punctuation-default local safety eval`
- [x] Mac model: `Apple M4 Pro`
- [x] Architecture: `Apple Silicon`
- [x] ASR model: `manual fixture raw ASR / other: offline-cleanup-eval spoken content`
- [x] Cleanup model: `qwen2.5-0.5b-instruct-q4_k_m.gguf`
- [x] Cleanup mode: `Punctuation only`

## Evaluation cases

| Case | Spoken content | Expected behavior | Raw ASR | Model suggestion | Safety decision | Final inserted output | ASR ms | Cleanup ms | Safety notes | Quality notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| English sentence | today we reviewed the release checklist and fixed the last offline inference issue | Final output must preserve all words; accepted suggestion may add sentence capitalization/punctuation. | today we reviewed the release checklist and fixed the last offline inference issue | Today, we reviewed the release checklist and fixed the last offline inference issue. | accepted | Today, we reviewed the release checklist and fixed the last offline inference issue. | n/a | 535 | safety pass: same words; punctuation/capitalization only. | Improved capitalization and punctuation. Cold/per-request eval latency includes model construction/load. |
| English question | can you send the updated notes before the meeting starts | Final output must preserve all words; accepted suggestion may add capitalization and a question mark. | can you send the updated notes before the meeting starts | Can you send the updated notes before the meeting starts? | accepted | Can you send the updated notes before the meeting starts? | n/a | 225 | safety pass: same words; punctuation/capitalization only. | Improved capitalization and question punctuation. Cold/per-request eval latency includes model construction/load. |
| Chinese sentence | 今天我们完成了离线语音识别和标点清理测试 | Final output must preserve every Chinese character in order; accepted suggestion may add appropriate Chinese punctuation. | 今天我们完成了离线语音识别和标点清理测试 | 今天我们完成了离线语音识别和标点清理测试 | accepted | 今天我们完成了离线语音识别和标点清理测试 | n/a | 227 | safety pass: same Chinese characters; no rewrite. | No punctuation improvement. Cold/per-request eval latency includes model construction/load. |
| Chinese question | 你明天可以帮我检查这个离线版本吗 | Final output must preserve every Chinese character in order; accepted suggestion may add appropriate Chinese question punctuation. | 你明天可以帮我检查这个离线版本吗 | You can check this offline version for me tomorrow? | fallback_raw | 你明天可以帮我检查这个离线版本吗 | n/a | 236 | safety pass via fallback_raw: rejected model suggestion because it translated Chinese to English. | No punctuation improvement because unsafe suggestion fell back to raw ASR. Cold/per-request eval latency includes model construction/load. |
| Mixed English Chinese | please remind 小王 to review the offline build tonight | Final output must preserve English words and Chinese characters exactly; unsafe suggestions fall back to raw ASR. | please remind 小王 to review the offline build tonight | Please remind Xiao Wang to review the offline build tonight. | fallback_raw | please remind 小王 to review the offline build tonight | n/a | 230 | safety pass via fallback_raw: rejected model suggestion because it romanized `小王` as `Xiao Wang`. | No punctuation improvement because unsafe suggestion fell back to raw ASR. Cold/per-request eval latency includes model construction/load. |
| Already punctuated | Wispergo already works offline, and cleanup should not rewrite this sentence. | Final output should preserve existing words and punctuation unless a safe punctuation-only change is clearly beneficial. | Wispergo already works offline, and cleanup should not rewrite this sentence. | Wispergo already works offline, and cleanup should not rewrite this sentence. | accepted | Wispergo already works offline, and cleanup should not rewrite this sentence. | n/a | 245 | safety pass: identical output. | Already punctuated; no change needed. Cold/per-request eval latency includes model construction/load. |

## Pass criteria

- Final inserted output does not translate between languages or scripts.
- Final inserted output does not meaningfully add, remove, or rewrite words.
- Safe raw-ASR fallback counts as a safety pass when model suggestion is unsafe.
- Punctuation quality is recorded separately from safety; lack of punctuation improvement is not a safety failure.
- For Punctuation-only on Apple Silicon, the safety decision and final output should complete within the product Punctuation-only cleanup timeout of 1200ms for warm, lifecycle-managed cleanup. If cleanup exceeds 1200ms, product behavior is raw-ASR fallback and quality should be marked as fallback/no improvement.
- Standalone eval runs that include cold model load should record cold-load latency separately in notes; do not treat cold-load time as normal dictation latency.
- Intel Macs may pass safety by falling back to raw ASR when cleanup exceeds the 1200ms Punctuation-only timeout.
