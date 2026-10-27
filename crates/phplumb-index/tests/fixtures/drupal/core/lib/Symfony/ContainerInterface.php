<?php
namespace Symfony\Component\DependencyInjection;

interface ContainerInterface
{
    public const NULL_ON_INVALID_REFERENCE = 2;

    public function get(string $id, int $invalidBehavior = 1): ?object;

    public function has(string $id): bool;
}
