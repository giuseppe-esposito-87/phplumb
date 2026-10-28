<?php

namespace App\Repository;

use Doctrine\Persistence\ObjectRepository;

class CommentRepository implements ObjectRepository
{
    public function find(mixed $id): ?object
    {
        return null;
    }

    public function findThread(): array
    {
        return [];
    }
}
