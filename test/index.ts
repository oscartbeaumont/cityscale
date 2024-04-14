import { connect } from "@planetscale/database";

let url = "";
url = "http://root;testing:admin@localhost:2489";

const conn = connect({
  url,
  fetch: async (input, init) => {
    console.log(input, init);
    const resp = await fetch(input, init);
    console.log(await resp.clone().text());
    return resp;
  },
});

// const results = await conn.execute("select 1, 2 from dual where 1=?", [1]);
// console.log(results);

// const results2 = await conn.execute("select 1, 2 from dual where 1=:id", {
//   id: 1,
// });
// console.log(results2);

// const results3 = await conn.transaction(async (tx) => {
//   const whenBranch = await tx.execute("SELECT 1");
//   const whenCounter = await tx.execute("SELECT 2");
//   return [whenBranch, whenCounter];
// });
// console.log(results3);

// const result2 = await conn.execute("SELECT * FROM test");
// console.log(result2);

const result = await conn.execute(
  "SELECT NULL as a, 1 as c, NULL as b, UNHEX('4d7953514c205475746f7269616c2c77337265736f757263') as d, CAST('2020-01-01 00:00:11.000012' AS DATETIME) as ee, UNIX_TIMESTAMP() as eee, DATE('2017-06-15 23:59:59') as e, DAY('2017-06-15') as f, CURTIME() as g, CAST('05:10:15' AS TIME) as gg, CAST('105:10:15' AS TIME) as ggg, CAST('-105:10:15' AS TIME) as gggg, CAST('105:10:15.5420' AS TIME) as ggggg from dual"
);
console.log(result);

// // TODO
// const result2 = await conn.execute("BRUH");
// console.log(result2);
