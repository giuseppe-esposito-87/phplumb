<?php
namespace Lib;

class Entity
{
    public function /*@target:entity_id*/id(): int
    {
        return 1;
    }
}

class Repo
{
    public function /*@target:find*/find(int $id): ?Entity
    {
        return null;
    }

    /** @return Entity[] */
    public function /*@target:all*/all(): array
    {
        return [];
    }
}

class Logger
{
    public function /*@target:log*/log(string $message): void
    {
    }
}

trait HasLogger
{
    /** @var Logger */
    protected /*@target:logger*/$logger;
}

abstract class BaseService
{
    /** @var Repo */
    protected /*@target:repo_prop*/$repo;

    /** @return static */
    public function /*@target:with_label*/withLabel(string $label)
    {
        return $this;
    }

    public function /*@target:with_flag*/withFlag(): self
    {
        return $this;
    }
}

class Service extends BaseService
{
    use HasLogger;

    public function /*@target:run*/run(): void
    {
    }

    public function /*@target:repo_method*/repo(): Repo
    {
        return $this->repo;
    }
}

class Foo
{
    public function /*@target:foo_bar*/bar(): void
    {
    }

    public function /*@target:m_both*/m(): void
    {
    }
}

class Baz
{
    public function /*@target:baz_qux*/qux(): void
    {
    }
}

class Bar
{
    public function /*@target:m_both*/m(): void
    {
    }
}

class Problem extends \Exception
{
    public function /*@target:problem_detail*/detail(): string
    {
        return '';
    }
}

function make_repo(): Repo
{
    return new Repo();
}
