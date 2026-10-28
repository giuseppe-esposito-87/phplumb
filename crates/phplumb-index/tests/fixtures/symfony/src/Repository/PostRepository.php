<?php

namespace App\Repository;

use Doctrine\Persistence\ObjectRepository;

class PostRepository implements ObjectRepository
{
    public function find(mixed $id): ?object
    {
        return null;
    }

    public function findRecent(): array
    {
        return [];
    }
}
