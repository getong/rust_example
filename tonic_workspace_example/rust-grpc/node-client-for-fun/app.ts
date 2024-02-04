import express, { Request, Response } from 'express';
import { createTodo, getTodos } from './services/todo';

const app = express();
app.use(express.json());

app.get('/todos', async (_: Request, res: Response) => {
    const { todos } = await getTodos({});

    res.json({ todos });
});

app.post('/todos', async (req: Request, res: Response) => {
    const { name, description, priority } = req.body;

    const { todo, status } = await createTodo({ name, description, priority });

    res.json({ todo, status });
});

const PORT = process.env.PORT || 8000;

app.listen(PORT, () => console.log(`Listening on port ${PORT}`));