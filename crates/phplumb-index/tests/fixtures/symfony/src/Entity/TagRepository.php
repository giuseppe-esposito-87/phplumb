<?php

namespace App\Entity;

use Doctrine\Persistence\ObjectRepository;

class TagRepository implements ObjectRepository
{
    public function find(mixed $id): ?object
    {
        return null;
    }

    public function findTagged(): array
    {
        return [];
    }
}
