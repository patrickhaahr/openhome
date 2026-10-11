import { defineApp, mutation, object, query, router, string } from "apps";
import { never } from "apps/operations/approval";
import { WishlistItem, WishlistItems } from "./schemas";

/** Read isolated demo entries; production wishlists will live in Axum. */
export const wishlist = query(
  { description: "Read the demo household wishlist", input: object({}), output: WishlistItems },
  async ({ sql }) =>
    sql
      .exec<{ id: string; owner: string; title: string }>(
        "SELECT id, owner, title FROM wishlist_items ORDER BY rowid DESC",
      )
      .toArray(),
);

/** Persist a generic demo wish. Caller-generated IDs make a repeated insert safe. */
export const addWish = mutation(
  {
    description: "Add a synthetic item to a demo user's wishlist; never touches a private account",
    input: object({
      id: string({ minLength: 1 }),
      owner: string({ minLength: 1 }),
      title: string({ minLength: 1 }),
    }),
    output: WishlistItem,
    approval: never(),
  },
  async ({ sql }, input) => {
    sql.exec(
      "INSERT OR IGNORE INTO wishlist_items (id, owner, title) VALUES (?, ?, ?)",
      input.id,
      input.owner,
      input.title,
    );
    return sql
      .exec<{ id: string; owner: string; title: string }>(
        "SELECT id, owner, title FROM wishlist_items WHERE id = ?",
        input.id,
      )
      .one();
  },
);

export default defineApp({ accounts: {} }, { tools: router({ wishlist, addWish }) });
