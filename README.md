# dataquery-workers

Cloudflare Workers(Rust) 기반의 **원격 데이터 조회 & JMESPath 질의** 유틸리티.

* URL의 응답을 가져와서(JSON), **JMESPath** 표현식으로 결과를 추출
* 응답 문자셋: `Content-Type`의 `charset`을 우선 사용, 없으면 `UTF-8`로 디코딩
* 결과는 `text/plain; charset=UTF-8`로 응답하며, `Cache-Control: max-age: 60` 헤더를 설정


## 엔드포인트

### `GET /`

상태 확인용.

응답(200)
```
dataquery
```

### `GET /raw/{input}/s/{*url}?<query>`

원본 요청을 가져와 “세 줄”로 에코합니다.

1. `:input` (URL-decoded)
2. `*url` 에 현재 요청의 query string을 이어붙인 최종 URL
3. 최종 URL의 HTTP GET 응답 본문

#### 예시

요청:
`/raw/input/s/https://example.com/api?foo=bar&qwe=asd`

응답:
```
input
https://example.com/api?foo=bar&qwe=asd
{"args": {"foo": "bar"}, ...}
```


### `GET /jp/{input}/s/{*url}?<query>`

* Alias: `GET /jmespath/{input}/s/{*url}?<query>`

최종 URL을 GET으로 호출하고, 응답(JSON 가정)에 대해 **JMESPath** 표현식(`:input`)을 실행한 결과를 텍스트로 반환합니다.

* 응답 본문은 jmespath::Variable의 to_string() 출력 (JSON 문자열 형태)
* 응답 인코딩은 다음 우선순위로 처리:
  1. Content-Type 헤더의 charset 디코딩
  2. (없으면) UTF-8 디코딩

#### 예시

요청:
`/jp/args.qwe/s/https://httpbin.org/get?foo=bar&qwe=asd`

응답(200):
```
"asd"
```


#### 주의

JMESPath 표현식(`:input`)에 /, ?, 공백, |, [, \` 같은 문자가 들어가면 URL 인코딩이 필요합니다.

예시)
```
/jp/result.areas[0].datas[?nv>`100000`]/s/...
```
→ ?는 `%3F`, \`는 `%60`으로 인코딩 후 경로에 포함

```
/jp/result.areas[0].datas[%3Fnv>%60100000%60]/s/...
```


## 로컬 실행 & 배포

### 개발 서버
```
npx wrangler dev
```

### 배포
```
npx wrangler deploy
```


## 제약/주의사항

* `/jp/...`, `/jmespath/...` 엔드포인트는 응답이 JSON이어야 정상 동작합니다.
HTML 등 JSON이 아니면 JSON 파싱 단계에서 실패합니다.

* `*url`은 디코딩하지 않고 그대로 사용합니다.

* 외부 URL 호출 정책(CORS와 무관, 서버→서버) 및 Cloudflare Workers의 네트워킹 제약은 계정/플랜 설정을 따릅니다.
