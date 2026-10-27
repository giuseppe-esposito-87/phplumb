<?php
namespace App;

trait Builds
{
    public function build(): void
    {
    }
}

class UsesAlias
{
    use Builds {
        build as traitBuild;
    }

    public function build(): void
    {
        $this->traitBuild();
        self::traitBuild();
        $this->/*@broken*/traitBuilt();
    }
}
