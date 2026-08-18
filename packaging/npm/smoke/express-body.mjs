
import express from "express";
const app = express();
app.use(express.json());
app.post("/echo", (req, res) => res.json({ got: req.body }));
app.listen(Number(process.argv[2]) || 18870, () => console.log("EXPRESS_BODY_READY"));
