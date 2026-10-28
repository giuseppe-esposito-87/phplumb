<?php

namespace Doctrine\Persistence;

interface ObjectRepository
{
    public function find(mixed $id): ?object;
}
