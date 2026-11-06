use std::fmt;
use std::option::Option;

pub struct LinkedList<T> {
    head: Option<Box<Node<T>>>,
    size: usize,
}

// 借用迭代器：只看着链表，产出 &T
pub struct LinkListIter<'a, T> {
    current: Option<&'a Node<T>>,
}

// 消耗迭代器：拥有整条链表，产出 T
pub struct IntoIter<T> {
    list: LinkedList<T>,
}

#[derive(Clone, PartialEq)]
struct Node<T> {
    value: T,
    next: Option<Box<Node<T>>>,
}

impl<T> Node<T> {
    pub fn new(value: T, next: Option<Box<Node<T>>>) -> Node<T> {
        Node {value: value, next: next}
    }
}

impl<T> LinkedList<T> {
    pub fn new() -> LinkedList<T> {
        LinkedList {head: None, size: 0}
    }
    
    pub fn get_size(&self) -> usize {
        self.size
    }
    
    pub fn is_empty(&self) -> bool {
        self.get_size() == 0
    }
    
    pub fn push_front(&mut self, value: T) {
        let new_node: Box<Node<T>> = Box::new(Node::new(value, self.head.take()));
        self.head = Some(new_node);
        self.size += 1;
    }
    
    pub fn pop_front(&mut self) -> Option<T> {
        let node: Box<Node<T>> = self.head.take()?;
        self.head = node.next;
        self.size -= 1;
        Some(node.value)
    }

    pub fn iter(&self) -> LinkListIter<'_, T> {
        LinkListIter {
            current: self.head.as_deref(),
        }
    }
}


impl<T> fmt::Display for LinkedList<T> where T: fmt::Display {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut current: &Option<Box<Node<T>>> = &self.head;
        let mut result = String::new();
        loop {
            match current {
                Some(node) => {
                    result = format!("{} {}", result, node.value);
                    current = &node.next;
                },
                None => break,
            }
        }
        write!(f, "{}", result)
    }
}

impl<T> Drop for LinkedList<T> {
    fn drop(&mut self) {
        let mut current = self.head.take();
        while let Some(mut node) = current {
            current = node.next.take();
        }
    }
}

impl<T> Clone for LinkedList<T> where T: Clone {
    fn clone(&self) -> LinkedList<T> {
        LinkedList {
            head: self.head.clone(),
            size: self.size,
        }
    }
}

impl<T> PartialEq for LinkedList<T> where T: PartialEq {
    fn eq(&self, other: &LinkedList<T>) -> bool {
        self.head == other.head
    }
}

// ----- Iter：借用，Item = &T -----
//for 循环调用 Iterator::next()
impl<'a, T> Iterator for LinkListIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<&'a T> {
        self.current.map(|node| {
            self.current = node.next.as_deref();
            &node.value
        })
    }
}

// for x in &list  
// for 循环调用 IntoIterator::into_iter()，生成 LinkListIter<'a, T> → Iterator::next()
impl<'a, T> IntoIterator for &'a LinkedList<T> {
    type Item = &'a T;
    type IntoIter = LinkListIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

// ----- IntoIter：消耗，Item = T -----
impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.list.pop_front()
    }
}

// for x in list 
impl<T> IntoIterator for LinkedList<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter { list: self }
    }
}
// depend on the Iterator trait
pub trait ComputeNorm {
    fn compute_norm(&self) -> f64;
}

impl ComputeNorm for LinkedList<f64> {
    fn compute_norm(&self) -> f64 {
        self.iter().map(|x| x * x).sum::<f64>().sqrt()
    }
}