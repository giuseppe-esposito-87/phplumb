<?php
namespace Symfony\Component\DependencyInjection;

interface ContainerInterface
{
    public function get(string $id, int $invalidBehavior = 1): ?object;
}
