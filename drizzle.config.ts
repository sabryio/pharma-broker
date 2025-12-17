import { defineConfig } from "drizzle-kit";
import dotenv from "dotenv";
dotenv.config({ path: ".env" });

const PB_DATABASE_DSN = process.env.PB_DATABASE_DSN;

if (!PB_DATABASE_DSN) {
  throw new Error("PB_DATABASE_DSN is not defined");
}

export default defineConfig({
  dialect: "postgresql",
  dbCredentials: {
    url: PB_DATABASE_DSN,
  },
});
