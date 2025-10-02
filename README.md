# dataquery-workers

Cloudflare Workers(Rust) + Axum 기반의 **원격 데이터 조회 & JMESPath 질의** 유틸리티.

* URL의 응답을 가져와서(JSON), **JMESPath** 표현식으로 결과를 추출
* 응답이 UTF-8이 아니어도 안전: `Content-Type`의 `charset`을 해석하고, 없으면 `UTF-8 → EUC-KR` 순으로 디코딩 시도


## 엔드포인트

### `GET /`

상태 확인용.

응답(200)
```
dataquery
```

### `GET /raw/{input}/s/{*url}?<query>`

원본 요청을 “두 줄”로 에코합니다.

```
1행: {input}
2행: {url}?<query>
```

#### 예시

요청:
`/raw/input/s/https://example.com/api?foo=bar&qwe=asd`

응답:
```
input
https://example.com/api?foo=bar&qwe=asd
```


### `GET /jp/{input}/s/{*url}?<query>`

`{url}` 에서 콘텐츠를 가져와(HTTP GET) **JMESPath** 표현식 `{input}` 으로 검색한 결과를 텍스트로 반환합니다.

* 응답 본문은 jmespath::Variable의 to_string() 출력(즉, JSON 값을 문자열로 직렬화한 결과)
* 응답 인코딩은 다음 우선순위로 처리:
  1. Content-Type 헤더의 charset 디코딩
  2. UTF-8 디코딩

#### 예시

요청:
`/jp/args.qwe/s/https://httpbin.org/get?foo=bar&qwe=asd`

응답(200):
```
"asd"
```


#### 주의

`{input}`(JMESPath 표현식)에 /, ?, 공백, |, [, \` 같은 문자가 들어가면 URL 인코딩이 필요합니다.

예시)
```
/jp/result.areas[0].datas[?nv>`100000`]/s/...
```
→ ?는 `%3F`, \`는 `%60`으로 인코딩 후 경로에 넣어주세요.

```
/jp/result.areas[0].datas[%3Fnv>%60100000%60]/s/...
```


## 로컬 실행 & 배포

### 개발 서버
```
wrangler dev
```

### 퍼블리시
```
wrangler publish
```


## 제약/주의사항

* `/jp/...` 엔드포인트는 응답이 JSON이어야 정상 동작합니다.
HTML 등 비-JSON을 받으면 JSON 파싱 단계에서 실패합니다.

* JMESPath 표현식은 URL 인코딩을 권장합니다.

* 외부 URL 호출 정책(CORS와 무관, 서버→서버) 및 Cloudflare Workers의 네트워킹 제약은 계정/플랜 설정을 따릅니다.
