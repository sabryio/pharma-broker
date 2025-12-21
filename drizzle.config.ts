import { defineConfig } from "drizzle-kit";
import dotenv from "dotenv";
dotenv.config({ path: ".env" });

const DATABASE_URL = process.env.DATABASE_URL || "postgres://postgres:password@localhost:5432/pharmabroker";

export default defineConfig({
  dialect: "postgresql",
  dbCredentials: {
    url: DATABASE_URL,
  },
});
