# Sales

| Region | Units | Revenue (currency) | Margin | Active | Start | Code (text) |
| --- | ---: | ---: | ---: | --- | --- | --- |
| North | 12 | $1,506.00 | 25% | true | 2026-01-15 | 001 |
| café 東京 | 9 | $1,260.00 | 20% | false | 2026-02-01 | 002 |
| Total | | | | | | |

```chart
{"type":"column","title":"Units by region","source":{"path":"self","sheet":"Sales","range":"A1:B3"},"options":{"anchor":"I2"}}
```

## Targets

| Region | Target (number) | Owner | Approved (boolean) |
| --- | ---: | --- | --- |
| North | 1500 | Alice | yes |
| South | 2000 | Bob | no |
