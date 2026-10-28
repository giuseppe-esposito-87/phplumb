<?php

namespace App\Entity;

class Post
{
    private int $id;

    private string $title;

    private Author $author;

    private Address $address;
}
