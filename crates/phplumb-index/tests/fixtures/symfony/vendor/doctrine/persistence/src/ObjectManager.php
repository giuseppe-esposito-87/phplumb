<?php

namespace Doctrine\Persistence;

interface ObjectManager
{
    public function getRepository(string $className): ObjectRepository;
}
