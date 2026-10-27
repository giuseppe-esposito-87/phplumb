<?php
namespace App;

use Lib\Bar;
use Lib\Baz;
use Lib\Foo;
use Lib\Problem;
use Lib\Repo;
use Lib\Service;
use function Lib\make_repo;

class Consumer extends Service
{
    /**
     * @param Repo $documented
     */
    public function go(Repo $typed, $documented, int $scalar, Foo|Bar $either): void
    {
        $this->/*@cursor:run*/run();
        $this->/*@cursor:logger*/logger->/*@cursor:log*/log('x');
        $this->/*@cursor:repo_prop*/repo->/*@cursor:find*/find(1);
        $this->/*@cursor:repo_method*/repo()->/*@cursor:find*/find(1)?->/*@cursor:entity_id*/id();
        $typed->/*@cursor:find*/find(1);
        $documented->/*@cursor:find*/find(1);
        make_repo()->/*@cursor:find*/find(2);

        $x = new Foo();
        $x->/*@cursor:foo_bar*/bar();
        $x = new Baz();
        $x->/*@cursor:baz_qux*/qux();

        $this->/*@cursor:with_label*/withLabel('a')->/*@cursor:run*/run();
        $this->/*@cursor:with_flag*/withFlag()->/*@cursor:with_label*/withLabel('b');

        foreach ($typed->/*@cursor:all*/all() as $entity) {
            $entity->/*@cursor:entity_id*/id();
        }

        /** @var Foo[] $items */
        $items = $this->collect();
        foreach ($items as $key => $item) {
            $item->/*@cursor:foo_bar*/bar();
        }

        try {
            $typed->/*@cursor:find*/find(3);
        } catch (Problem $e) {
            $e->/*@cursor:problem_detail*/detail();
        }

        /** @var Baz $inline */
        $inline = $this->unknown();
        $inline->/*@cursor:baz_qux*/qux();

        $maybe = $this->unknown();
        if ($maybe instanceof Foo) {
            $maybe->/*@cursor:foo_bar*/bar();
        }

        $either->/*@cursor:m_both*/m();

        $unknown = $this->unknown();
        $unknown->/*@cursor:none*/run();
        $this->/*@cursor:none*/missing();
        $scalar->/*@cursor:none*/foo();
        $name = 'run';
        $this->/*@cursor:none*/$name();
    }
}
