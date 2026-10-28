<?php

namespace App\Service;

use App\Entity\Author;
use App\Entity\Comment;
use App\Entity\Ghost;
use App\Entity\Post;
use App\Entity\Tag;
use Doctrine\Persistence\ObjectManager;

final class Posts
{
    public function __construct(private ObjectManager $em)
    {
    }

    public function load(string $class): void
    {
        $this->em->getRepository(Post::class)->findRecent();
        $this->em->getRepository(Comment::class)->findThread();
        $this->em->getRepository(Author::class)->find(1);
        $this->em->getRepository($class)->find(1);
        $this->em->getRepository(Tag::class)->findTagged();
        $this->em->getRepository(Ghost::class)->find(1);
    }
}
