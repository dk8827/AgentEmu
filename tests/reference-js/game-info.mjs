import { GAMES } from "./game-catalog.mjs";
// Exact local cartridge identities; filenames never determine controller capability.
export const SOLO_GAMES = {
  "6ba64ff651ede7eea2f6142f681b5f03df496456523e3712f8cd5bbee8f2f2ae": {
    title: "Super Mario Bros. 2",
    players: 1,
    actions: ["Jump", "Run / pick up"],
    battery: false,
  },
  "89232edf4f9b52e3cb872094bc78973de080befca2ddea893b6e936066514d4e": {
    title: "The Legend of Zelda",
    players: 1,
    actions: ["Sword", "Item"],
    battery: true,
  },
  ad8c0fbcf092bf84b48e69fd3964eea4ed91bfe62abc352943d537979782680c: {
    title: "Zelda II: The Adventure of Link",
    players: 1,
    actions: ["Jump", "Attack"],
    battery: true,
  },
  c5eea06e1e1128b576bd789f1a4f63bb154d6d31579c2f319382fb77a72d34a6: {
    title: "Metroid",
    players: 1,
    actions: ["Jump", "Fire"],
    battery: false,
  },
  "27b5a635df33ed57ed339dfc7fd62fc603b39c1d1603adb5cdc3562a0b0d555b": {
    title: "Mega Man 2",
    players: 1,
    actions: ["Jump", "Fire"],
    battery: false,
  },
  "0bf466eb3819eee34e41de076b5c1bf16375d0de5ba64ede8ade6ed599accb53": {
    title: "Castlevania",
    players: 1,
    actions: ["Jump", "Whip"],
    battery: false,
  },
  "07d9e57e35184667c7b89e30857a44c5c404b48332ad12e66be91100eccd4f5e": {
    title: "Ninja Gaiden",
    players: 1,
    actions: ["Jump", "Sword"],
    battery: false,
  },
  "0435f6d9bcfeb474a592c3a950530a99c03a6f6cacff793dc88f822bb8cc6952": {
    title: "Mike Tyson\u2019s Punch-Out!!",
    players: 1,
    actions: ["Right punch", "Left punch"],
    battery: false,
  },
  "34afcebaf04d57a42c2436fb95b933c7841e1621f1dc3be482380baef33ec3d8": {
    title: "Kirby\u2019s Adventure",
    players: 1,
    actions: ["Jump", "Inhale / ability"],
    battery: true,
  },
  d360beeee19a69d1c92d02292167c57f2d3ca8405b79101fc8c2f39dea549667: {
    title: "Final Fantasy",
    players: 1,
    actions: ["Confirm", "Back"],
    battery: true,
  },
};
export const gameInfo = (hash) => SOLO_GAMES[hash] ?? GAMES[hash] ?? null;
